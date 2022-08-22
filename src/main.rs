mod mutate;
mod playersearch;

use tf_demo_parser::{Demo};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::{RawPacketStream, DemoHandler, Encode};
use tf_demo_parser::demo::packet::{PacketType};
use tf_demo_parser::demo::message::Message;
use bitbuffer::{BitWriteStream, LittleEndian, BitRead, BitWrite};
use tf_demo_parser::demo::message::packetentities::{EntityId, PacketEntity};
use tf_demo_parser::demo::sendprop::{SendPropIdentifier, SendPropValue};
use std::fs;
use tf_demo_parser::demo::message::usermessage::UserMessageType;
use crate::mutate::{MessageFilter, Mutator, MutatorList, PacketFilter};
use clap::Parser;
use crate::playersearch::get_player;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Demo to strip
    path: String,
    /// Name or steam id of the player to strip
    user: Option<String>,
}

fn main() {
    let args = Args::parse();
    let file = fs::read(&args.path).unwrap();
    let out_path = format!("{}_no_uber.dem", args.path.trim_end_matches(".dem"));

    let stripped = mutate(&file, args.user.map(|user| user.to_ascii_lowercase()));
    fs::write(out_path, stripped).unwrap();
}

fn mutate(input: &[u8], user: Option<String>) -> Vec<u8> {
    let mut out_buffer = Vec::with_capacity(input.len());
    {
        let mut out_stream = BitWriteStream::new(&mut out_buffer, LittleEndian);

        let demo = Demo::new(&input);
        let mut stream = demo.get_stream();
        let header = Header::read(&mut stream).unwrap();
        header.write(&mut out_stream).unwrap();

        let mut mutator = MutatorList::new();
        mutator.push(MessageFilter::new(|message| {
            if let Message::UserMessage(usr_message) = message {
                UserMessageType::CloseCaption != usr_message.message_type()
            } else {
                true
            }
        }));
        mutator.push(PacketFilter::new(|packet| {
            packet.packet_type() != PacketType::ConsoleCmd
        }));
        mutator.push({
            let mut mask = CondMask::new(get_player(&demo, user).0);
            mask.remove_cond(5); // uber
            mask.remove_cond(8); // uber wearing off
            mask.remove_cond(28); // qf
            mask.remove_cond(11); // kritz
            mask.remove_cond(24); // jarate
            mask.remove_cond(25); // bleed
            mask
        });

        let mut packets = RawPacketStream::new(stream.clone());
        let mut handler = DemoHandler::default();
        handler.handle_header(&header);

        while let Some(mut packet) = packets.next(&handler.state_handler).unwrap() {
            if mutator.filter_packet(&packet) {
                mutator.mutate_packet(&mut packet);
                packet
                    .encode(&mut out_stream, &handler.state_handler)
                    .unwrap();
            }
            handler.handle_packet(packet).unwrap();
        }
    }
    out_buffer
}

struct CondMask {
    cond: i64,
    entity: EntityId,
}

impl CondMask {
    pub fn new(entity: EntityId) -> Self {
        if entity == 1 {
            eprintln!("Attempting to strip stv demo without specifying a player")
        }
        CondMask {
            cond: i64::MAX,
            entity,
        }
    }

    pub fn remove_cond(&mut self, cond: u8) {
        self.cond &= !(1 << cond);
    }
}

const PROP_ID: SendPropIdentifier = SendPropIdentifier::new("DT_TFPlayerShared", "m_nPlayerCond");

impl Mutator for CondMask {
    fn mutate_entity(&self, entity: &mut PacketEntity) {
        if entity.entity_index == self.entity {
            entity.props.iter_mut().filter(|prop| prop.identifier == PROP_ID).for_each(|prop| {
                if let SendPropValue::Integer(value) = &mut prop.value {
                    *value &= self.cond;
                }
            })
        }
    }
}
