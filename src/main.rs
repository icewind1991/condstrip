mod mutate;

use tf_demo_parser::{Demo};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::{RawPacketStream, DemoHandler, Encode};
use tf_demo_parser::demo::packet::{Packet, PacketType};
use tf_demo_parser::demo::message::Message;
use bitbuffer::{BitWriteStream, LittleEndian, BitRead, BitWrite};
use tf_demo_parser::demo::message::packetentities::{EntityId, PacketEntity};
use tf_demo_parser::demo::sendprop::{SendPropIdentifier, SendPropValue};
use std::env::args;
use std::fs;
use tf_demo_parser::demo::message::usermessage::UserMessageType;
use tf_demo_parser::demo::packet::message::MessagePacket;
use crate::mutate::{MessageFilter, Mutator, MutatorList, PacketFilter};

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

    let stripped = mutate(&file);
    fs::write(out_path, stripped).unwrap();
}

fn mutate(input: &[u8]) -> Vec<u8> {
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
            let mut mask = CondMask::new(dbg!(get_player(&demo)));
            mask.remove_cond(5); // uber
            mask.remove_cond(8); // uber wearing off
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

struct CondMask{
    cond: i64,
    entity: EntityId
}

impl CondMask {
    pub fn new(entity: EntityId) -> Self {
        CondMask{
            cond: i64::MAX,
            entity
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

fn get_player(demo: &Demo) -> EntityId {
    let mut stream = demo.get_stream();
    let header = Header::read(&mut stream).unwrap();

    let mut packets = RawPacketStream::new(stream.clone());
    let mut handler = DemoHandler::default();
    handler.handle_header(&header);

    while let Some(packet) = packets.next(&handler.state_handler).unwrap() {
        if let Packet::Signon(MessagePacket{messages, ..}) = &packet {
            for message in messages {
                if let Message::ServerInfo(info) = message {
                    return EntityId::from(info.player_slot as u32 + 1);
                }
            }
        }
        handler.handle_packet(packet).unwrap();
    }
    panic!("no server info");
}