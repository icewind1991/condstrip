mod mutate;

use tf_demo_parser::{Demo, DemoParser, MessageType, ParserState};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::{RawPacketStream, DemoHandler, Encode, MessageHandler};
use tf_demo_parser::demo::packet::{PacketType};
use tf_demo_parser::demo::message::Message;
use bitbuffer::{BitWriteStream, LittleEndian, BitRead, BitWrite};
use tf_demo_parser::demo::message::packetentities::{EntityId, PacketEntity};
use tf_demo_parser::demo::sendprop::{SendPropIdentifier, SendPropValue};
use std::fs;
use tf_demo_parser::demo::message::usermessage::UserMessageType;
use crate::mutate::{MessageFilter, Mutator, MutatorList, PacketFilter};
use clap::Parser;
use tf_demo_parser::demo::data::UserInfo;
use tf_demo_parser::demo::packet::stringtable::{StringTableEntry};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    path: String,
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
            let mut mask = CondMask::new(dbg!(get_player(&demo, user)));
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

fn get_player(demo: &Demo, user: Option<String>) -> EntityId {
    let parser = DemoParser::new_with_analyser(demo.get_stream(), PlayerSearchHandler::new(user));

    parser.parse().expect("failed to parse demo").1.expect("no server info or player not found")
}

struct PlayerSearchHandler {
    user: Option<String>,
    entity: Option<EntityId>,
}

impl PlayerSearchHandler {
    pub fn new(user: Option<String>) -> Self {
        PlayerSearchHandler {
            user,
            entity: None,
        }
    }
}

impl MessageHandler for PlayerSearchHandler {
    type Output = Option<EntityId>;

    fn does_handle(_message_type: MessageType) -> bool {
        true
    }

    fn handle_message(&mut self, message: &Message, _tick: u32, _parser_state: &ParserState) {
        if self.user.is_none() {
            if let Message::ServerInfo(info) = message {
                self.entity = Some(EntityId::from(info.player_slot as u32 + 1));
            }
        }
    }


    fn handle_string_entry(&mut self, table: &str, _index: usize, entry: &StringTableEntry, _parser_state: &ParserState) {
        if let Some(user) = self.user.as_deref() {
            if table == "userinfo" {
                if let Ok(Some(info)) = UserInfo::parse_from_string_table(entry.text.as_deref(), entry.extra_data.as_ref().map(|data| data.data.clone())) {
                    if info.player_info.name.to_ascii_lowercase().contains(user) {
                        self.entity = Some(info.entity_id);
                    }
                }
            }
        }
    }

    fn into_output(self, _state: &ParserState) -> Self::Output {
        self.entity
    }
}