use crate::scheduler::Scheduler;
use rverilog_mir::LogicVal;

pub struct SysTaskExecutor {
    scheduler: Scheduler,
}

impl SysTaskExecutor {
    pub fn new(scheduler: Scheduler) -> Self {
        SysTaskExecutor { scheduler }
    }

    pub fn display(&mut self, args: &[LogicVal]) {
        // TODO: Implement $display
        let msg = format!("{:?}", args);
        println!("{}", msg);
    }

    pub fn write(&mut self, args: &[LogicVal]) {
        // TODO: Implement $write
        let msg = format!("{:?}", args);
        print!("{}", msg);
    }

    pub fn monitor(&mut self) {
        // TODO: Implement $monitor
    }

    pub fn finish(&mut self) {
        // TODO: Implement $finish
        std::process::exit(0);
    }

    pub fn time(&self) -> u64 {
        self.scheduler.now()
    }

    pub fn dump_file(&mut self, path: &str) {
        // TODO: Implement $dumpfile
        println!("Dump to: {}", path);
    }

    pub fn dump_vars(&mut self, level: i32) {
        // TODO: Implement $dumpvars
        println!("Dump vars at level: {}", level);
    }
}
