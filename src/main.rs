use tf_demo_parser::{Demo};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::{RawPacketStream, DemoHandler, Encode};
use tf_demo_parser::demo::packet::{Packet};
use tf_demo_parser::demo::message::Message;
use bitbuffer::{BitWriteStream, LittleEndian, BitRead, BitWrite};
use tf_demo_parser::demo::message::packetentities::PacketEntity;
use tf_demo_parser::demo::sendprop::{SendPropIdentifier, SendPropValue};
use std::env::args;
use std::fs;

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
    let mutator = {
        let mut mask = CondMask::new();
        mask.remove_cond(5);
        mask
    };

    let stripped = mutate(&file, &mutator);
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
            packet
                .encode(&mut out_stream, &handler.state_handler)
                .unwrap();
            handler.handle_packet(packet).unwrap();
        }
    }
    out_buffer
}

trait PacketMutator {
    fn mutate_packet(&self, packet: &mut Packet);
}

impl<T: MessageMutator> PacketMutator for T {
    fn mutate_packet(&self, packet: &mut Packet) {
        if let Packet::Message(msg_packet) = packet {
            msg_packet.messages.iter_mut().for_each(|msg| self.mutate_message(msg));
        }
    }
}

trait MessageMutator {
    fn mutate_message(&self, message: &mut Message);
}

impl<T: EntityMutator> MessageMutator for T {
    fn mutate_message(&self, message: &mut Message) {
        if let Message::PacketEntities(entity_message) = message {
            entity_message.entities.iter_mut().for_each(|ent| self.mutate_entity(ent))
        }
    }
}

trait EntityMutator {
    fn mutate_entity(&self, entity: &mut PacketEntity);
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