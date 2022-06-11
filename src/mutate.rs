use std::mem::take;
use tf_demo_parser::demo::message::packetentities::PacketEntity;
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::demo::packet::Packet;

pub trait Mutator {
    fn filter_packet(&self, _packet: &Packet) -> bool {
        true
    }

    fn mutate_packet(&self, packet: &mut Packet) {
        if let Packet::Message(msg_packet) = packet {
            let messages = take(&mut msg_packet.messages);
            msg_packet.messages = messages
                .into_iter()
                .filter(|msg| self.filter_message(msg))
                .map(|mut msg| {
                    self.mutate_message(&mut msg);
                    msg
                })
                .collect();
        }
    }

    fn mutate_message(&self, message: &mut Message) {
        if let Message::PacketEntities(entity_message) = message {
            entity_message.entities.iter_mut().for_each(|ent| self.mutate_entity(ent))
        }
    }

    fn mutate_entity(&self, _entity: &mut PacketEntity) {

    }

    fn filter_message(&self, _message: &Message) -> bool {
        true
    }
}

pub struct PacketFilter<F: Fn(&Packet) -> bool>(F);

impl<F: Fn(&Packet) -> bool> PacketFilter<F> {
    pub fn new(f: F) -> Self<> {
        Self(f)
    }
}

impl<F: Fn(&Packet) -> bool> Mutator for PacketFilter<F> {
    fn filter_packet(&self, packet: &Packet) -> bool {
        self.0(packet)
    }
}

pub struct MessageFilter<F: Fn(&Message) -> bool>(F);

impl<F: Fn(&Message) -> bool> MessageFilter<F> {
    pub fn new(f: F) -> Self<> {
        Self(f)
    }
}

impl<F: Fn(&Message) -> bool> Mutator for MessageFilter<F> {
    fn filter_message(&self, message: &Message) -> bool {
        self.0(message)
    }
}

pub struct MessageMutator<F: Fn(&mut Message)>(F);

impl<F: Fn(&mut Message)> MessageMutator<F> {
    #[allow(dead_code)]
    pub fn new(f: F) -> Self<> {
        Self(f)
    }
}

impl<F: Fn(&mut Message)> Mutator for MessageMutator<F> {
    fn mutate_message(&self, message: &mut Message) {
        self.0(message)
    }
}

#[derive(Default)]
pub struct MutatorList {
    mutators: Vec<Box<dyn Mutator>>,
}

impl MutatorList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push<M: Mutator + 'static>(&mut self, mutator: M) {
        self.mutators.push(Box::new(mutator))
    }
}

impl Mutator for MutatorList {
    fn filter_packet(&self, packet: &Packet) -> bool {
        for mutator in self.mutators.iter() {
            if !mutator.filter_packet(packet) {
                return false;
            }
        }
        true
    }

    fn mutate_packet(&self, packet: &mut Packet) {
        for mutator in self.mutators.iter() {
            mutator.mutate_packet(packet);
        }
    }
}
