#![feature(asm)]
#![feature(naked_functions)]
use std::ptr;

const DEFAULT_STACK_SIZE: usize = 1024 * 1024* 2;
const MAX_THREADS: usize = 4;
static mut RUNTIME: usize = 0;

pub struct Runtime {
    threads: Vec<Thread>,
    current: usize,
}

#[derive(PartialEq, Eq, Debug)]
enum State {
    Available,
    Running,
    Ready,
}

struct Thread {
    id: usize,
    stack: Vec<u8>,
    ctx: ThreadContext,
    state: State,
}

#[derive(Debug, Default)]
#[repr(C)] 
struct ThreadContext {
    rsp: u64,
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
    win_nt_tib: u128,
}

impl Thread {
    fn new(id: usize) -> Self {
        Thread {
            id,
            stack: vec![0_u8; DEFAULT_STACK_SIZE],
            ctx: ThreadContext::default(),
            state: State::Available,
        }
    }
}

impl Runtime {
    pub fn new() -> Self {
        let base_thread = Thread {
            id: 0,
            stack: vec![0_u8; DEFAULT_STACK_SIZE],
            ctx: ThreadContext::default(),
            state: State::Running,
        };

        let mut threads = vec![base_thread];
        let mut available_threads: Vec<Thread> = (1..MAX_THREADS).map(|i| Thread::new(i)).collect();
        threads.append(&mut available_threads);

        Runtime {
            threads,
            current: 0,
        }
    }

    pub fn init(&self) {
        unsafe {
            let r_ptr: *const Runtime = self;
            RUNTIME = r_ptr as usize;
        }
    }

    pub fn run(&mut self) -> ! {
        while self.t_yield() {}
        std::process::exit(0);
    }

    fn t_return(&mut self) {
        if self.current != 0 {
            self.threads[self.current].state = State::Available;
            self.t_yield();
        }
    }
    
    fn t_yield(&mut self) -> bool {
        let mut pos = self.current;
        while self.threads[pos].state != State::Ready {
            pos += 1;
            if pos == self.threads.len() {
                pos = 0;
            }
            if pos == self.current {
                return false;
            }
        }

        if self.threads[self.current].state != State::Available {
            self.threads[self.current].state = State::Ready;
        }

        self.threads[pos].state = State::Running;
        let old_pos = self.current;
        self.current = pos;

        unsafe {
            switch(&mut self.threads[old_pos].ctx, &self.threads[pos].ctx);
        }

        true
    }

    pub fn spawn(&mut self, f: fn()) {
        let available = self
            .threads
            .iter_mut()
            .find(|t| t.state == State::Available)
            .expect("no available thread.");

        let size = available.stack.len();
        let s_ptr = available.stack.as_mut_ptr();

        unsafe {
            ptr::write(s_ptr.offset((size - 8) as isize) as *mut u64, guard as u64);
            ptr::write(s_ptr.offset((size - 16) as isize) as *mut u64, f as u64);
            available.ctx.rsp = s_ptr.offset((size - 16) as isize) as u64;
        }
        available.state = State::Ready;
    }
}

#[cfg_attr(any(target_os="windows", target_os="linux"), naked)]
fn guard() {
    unsafe {
        let rt_ptr = RUNTIME as *mut Runtime;
        let rt = &mut *rt_ptr;
        println!("THREAD {} FINISHED.", rt.threads[rt.current].id);
        rt.t_return();
    };
}

pub fn yield_thread() {
    unsafe {
        let rt_ptr = RUNTIME as *mut Runtime;
        (*rt_ptr).t_yield();
    };
}

// see: https://github.com/rust-lang/rfcs/blob/master/text/1201-naked-fns.md
#[naked]
unsafe fn switch(old: *mut ThreadContext, new: *const ThreadContext) {

    // if cfg!(target_os = "windows") {

    // }

    asm!("
        movq     $0, %rdi
        movq     %rsp, 0x00(%rdi)
        movq     %r15, 0x08(%rdi)
        movq     %r14, 0x10(%rdi)
        movq     %r13, 0x18(%rdi)
        movq     %r12, 0x20(%rdi)
        movq     %rbx, 0x28(%rdi)
        movq     %rbp, 0x30(%rdi)
        movq     %gs:0x08, %rax
        movq     %rax, 0x38(%rdi)
        movq     %gs:0x16, %rax
        movq     %rax, 0x40(%rdi)

        movq     $1, %rsi
        movq     0x00(%rsi), %rsp
        movq     0x08(%rsi), %r15
        movq     0x10(%rsi), %r14
        movq     0x18(%rsi), %r13
        movq     0x20(%rsi), %r12
        movq     0x28(%rsi), %rbx
        movq     0x30(%rsi), %rbp
        movq     0x38(%rdi), %rax
        movq     %rax, %gs:0x08
        movq     0x40(%rdi), %rax
        movq     %rax, %gs:0x16

        retq
        "
    :
    :"r"(old), "r"(new)
    : "rdi"
    : "volatile", "alignstack"
    );

}

fn main() {
    let mut runtime = Runtime::new();
    runtime.init();
    runtime.spawn(|| {
        println!("THREAD 1 STARTING");
        let id = 1;
        for i in 0..10 {
            println!("thread: {} counter: {}", id, i);
            yield_thread();
        }
    });
    runtime.spawn(|| {
        println!("THREAD 2 STARTING");
        let id = 2;
        for i in 0..15 {
            println!("thread: {} counter: {}", id, i);
            yield_thread();
        }
    });
    runtime.run();
}