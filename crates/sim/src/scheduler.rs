use rverilog_mir::{NetId, ProcessId, ContId, LogicVal};
use std::collections::VecDeque;
use std::cmp::Reverse;

#[derive(Debug)]
pub struct Scheduler {
    now: u64,
    future: VecDeque<Reverse<(u64, u32, Event)>>,
    active: VecDeque<Event>,
    inactive: VecDeque<Event>,
    nba: VecDeque<NbaUpdate>,
    seq: u32,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub time: u64,
    pub seq: u32,
    pub event: EventType,
}

#[derive(Debug, Clone)]
pub enum EventType {
    ResumeProcess(ProcessId, Pc),
    Update(NetId, LogicVal),
    EvalCont(ContId),
    Wake(ProcessId),
}

#[derive(Debug)]
pub struct NbaUpdate {
    pub net: NetId,
    pub value: LogicVal,
    pub time: u64,
}



#[derive(Debug, Clone, Copy)]
pub struct Pc(#[allow(dead_code)] usize);

impl Scheduler {
    pub fn new() -> Self {
        Scheduler {
            now: 0,
            future: VecDeque::new(),
            active: VecDeque::new(),
            inactive: VecDeque::new(),
            nba: VecDeque::new(),
            seq: 0,
        }
    }

    pub fn now(&self) -> u64 {
        self.now
    }

    pub fn push_active(&mut self, event: Event) {
        self.active.push_back(event);
    }

    pub fn push_inactive(&mut self, event: Event) {
        self.inactive.push_back(event);
    }

    pub fn push_nba(&mut self, update: NbaUpdate) {
        self.nba.push_back(update);
    }

    pub fn schedule(&mut self, time: u64, event: EventType) {
        self.seq += 1;
        let evt = Event { time, seq: self.seq, event };
        self.future.push_back(Reverse((time, self.seq, evt)));
    }

    pub fn run_step(&mut self) -> bool {
        if self.active.is_empty() {
            if !self.inactive.is_empty() {
                std::mem::swap(&mut self.active, &mut self.inactive);
            } else if !self.nba.is_empty() {
                self.apply_nba();
            } else if !self.future.is_empty() {
                self.advance_time();
            } else {
                return false;
            }
        }

        if let Some(event) = self.active.pop_front() {
            match event.event {
                EventType::ResumeProcess(_, _) => {
                    // TODO: Resume process
                }
                EventType::Update(_net, _val) => {
                }
                EventType::EvalCont(_) => {
                    // TODO: Evaluate continuous assignment
                }
                EventType::Wake(_) => {
                    // TODO: Wake process
                }
            }
        }

        true
    }

    fn apply_nba(&mut self) {
        for update in self.nba.drain(..) {
            self.active.push_back(Event {
                time: self.now,
                seq: self.seq,
                event: EventType::Update(update.net, update.value),
            });
        }
    }

    fn advance_time(&mut self) {
        if let Some(Reverse((time, _, event))) = self.future.pop_front() {
            self.now = time;
            self.active.push_back(event);
            // Move events at current time from future to active
            while let Some(Reverse((t, _, e))) = self.future.pop_front() {
                if t == self.now {
                    self.active.push_back(e);
                } else {
                    self.future.push_front(Reverse((t, 0, e)));
                    break;
                }
            }
        }
    }
}
