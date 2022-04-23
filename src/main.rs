mod mutate;

use tf_demo_parser::{Demo};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::{RawPacketStream, DemoHandler, Encode};
use tf_demo_parser::demo::packet::{PacketType};
use tf_demo_parser::demo::message::Message;
use bitbuffer::{BitWriteStream, LittleEndian, BitRead, BitWrite};
use tf_demo_parser::demo::message::packetentities::PacketEntity;
use tf_demo_parser::demo::sendprop::{SendPropIdentifier, SendPropValue};
use std::env::args;
use std::fs;
use tf_demo_parser::demo::message::usermessage::UserMessageType;
use crate::mutate::{EntityMutator, MutatorList, PacketMutator};

fn main() {
    let mut args = args();
    let bin = args.next().unwrap();
    let path = match args.next() {
        Some(file) => file,
        None => {
            println!(
                "usage: {} <demo>",
                bin
            );
            return;
        }
    };
    let file = fs::read(&path).unwrap();
    let out_path = format!("{}_no_uber.dem", path.trim_end_matches(".dem"));

    let mut mutators = MutatorList::new();
    mutators.push_message_filter(|message: &Message| {
        if let Message::UserMessage(usr_message) = message {
            UserMessageType::CloseCaption != usr_message.message_type()
        } else {
            true
        }
    });
    mutators.push_entity_mutator({
        let mut mask = CondMask::new();
        mask.remove_cond(5);
        mask
    });

    let stripped = mutate(&file, &mutators);
    fs::write(out_path, stripped).unwrap();
}

fn mutate<M: PacketMutator>(input: &[u8], mutator: &M) -> Vec<u8> {
    let mut out_buffer = Vec::with_capacity(input.len());
    {
        let mut out_stream = BitWriteStream::new(&mut out_buffer, LittleEndian);

        let demo = Demo::new(&input);
        let mut stream = demo.get_stream();
        let header = Header::read(&mut stream).unwrap();
        header.write(&mut out_stream).unwrap();

        let mut packets = RawPacketStream::new(stream.clone());
        let mut handler = DemoHandler::default();
        handler.handle_header(&header);

        while let Some(mut packet) = packets.next(&handler.state_handler).unwrap() {
            mutator.mutate_packet(&mut packet);
            if packet.packet_type() != PacketType::ConsoleCmd {
                packet
                    .encode(&mut out_stream, &handler.state_handler)
                    .unwrap();
            }
            handler.handle_packet(packet).unwrap();
        }
    }
    out_buffer
}

struct CondMask(i64);

impl CondMask {
    pub fn new() -> Self {
        CondMask(i64::MAX)
    }

    pub fn remove_cond(&mut self, cond: u8) {
        self.0 &= !(1 >> cond);
    }
}

const PROP_ID: SendPropIdentifier = SendPropIdentifier::new("DT_TFPlayerShared", "m_nPlayerCond");

impl EntityMutator for CondMask {
    fn mutate_entity(&self, entity: &mut PacketEntity) {
        entity.props.iter_mut().filter(|prop| prop.identifier == PROP_ID).for_each(|prop| {
            if let SendPropValue::Integer(value) = &mut prop.value {
                *value &= self.0;
            }
        })
    }
}