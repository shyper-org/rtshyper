use spin::RwLock;

use crate::board::PLAT_DESC;
use crate::lib::round_up;

struct CoreBarrier {
    count: u32,
}

static BARRIER: RwLock<CoreBarrier> = RwLock::new(CoreBarrier { count: 0 });

pub fn barrier() {
    let next_count;
    let mut barrier = BARRIER.write();
    barrier.count += 1;
    next_count = round_up(barrier.count as usize, PLAT_DESC.cpu_desc.num);
    drop(barrier);
    loop {
        if BARRIER.read().count as usize >= next_count {
            break;
        }
    }
}
