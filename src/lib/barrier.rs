use spin::Mutex;

use crate::board::PLAT_DESC;
use crate::lib::round_up;

struct CoreBarrier {
    count: u32,
}

static BARRIER: Mutex<CoreBarrier> = Mutex::new(CoreBarrier { count: 0 });

pub fn barrier() {
    let next_count;
    let mut barrier = BARRIER.lock();
    barrier.count += 1;
    next_count = round_up(barrier.count as usize, PLAT_DESC.cpu_desc.num);
    drop(barrier);
    loop {
        if BARRIER.lock().count as usize >= next_count {
            break;
        }
    }
}
