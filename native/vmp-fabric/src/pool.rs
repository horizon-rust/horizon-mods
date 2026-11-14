// Port of com.ishland.vmp.common.util.SimpleObjectPool (SimpleObjectPool.java).
// A fixed-size pool that pre-constructs `size` objects and hands them out;
// alloc() past the size falls back to a plain construction (no initializer),
// and release() is a no-op once every pooled object is already out.
// Upstream guards the array with synchronized blocks; the port keeps the
// same state machine for single-threaded use.

pub struct SimpleObjectPool<T> {
    constructor: Box<dyn Fn() -> T>,
    initializer: Box<dyn Fn(&mut T)>,
    on_recycle: Box<dyn Fn(&mut T)>,
    size: usize,
    cached: Vec<Option<T>>,
    allocated_count: usize,
}

impl<T> SimpleObjectPool<T> {
    pub fn new(
        constructor: Box<dyn Fn() -> T>,
        initializer: Box<dyn Fn(&mut T)>,
        on_recycle: Box<dyn Fn(&mut T)>,
        size: usize,
    ) -> Self {
        assert!(size > 0, "size must be positive");
        Self {
            constructor,
            initializer,
            on_recycle,
            size,
            cached: Vec::new(),
            allocated_count: 0,
        }
    }

    fn init(&mut self) {
        if self.cached.is_empty() {
            self.cached = (0..self.size).map(|_| Some((self.constructor)())).collect();
        }
    }

    pub fn alloc(&mut self) -> T {
        self.init();
        if self.allocated_count >= self.size {
            // oversized, falling back to a normal alloc
            return (self.constructor)();
        }
        let ordinal = self.allocated_count;
        self.allocated_count += 1;
        let mut object = self.cached[ordinal].take().expect("pooled slot");
        (self.initializer)(&mut object);
        object
    }

    pub fn release(&mut self, mut object: T) {
        self.init();
        if self.allocated_count == 0 {
            return; // pool is full
        }
        (self.on_recycle)(&mut object);
        self.allocated_count -= 1;
        self.cached[self.allocated_count] = Some(object);
    }

    pub fn get_allocated_count(&self) -> usize {
        self.allocated_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counting_pool(
        size: usize,
    ) -> (
        SimpleObjectPool<Vec<u32>>,
        std::rc::Rc<std::cell::Cell<usize>>,
    ) {
        let constructed = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let c = constructed.clone();
        let pool = SimpleObjectPool::new(
            Box::new(move || {
                c.set(c.get() + 1);
                Vec::new()
            }),
            Box::new(|v: &mut Vec<u32>| v.push(1)),
            Box::new(|v: &mut Vec<u32>| v.clear()),
            size,
        );
        (pool, constructed)
    }

    #[test]
    fn alloc_reuses_released_objects() {
        let (mut pool, constructed) = counting_pool(2);
        let a = pool.alloc();
        let b = pool.alloc();
        assert_eq!(pool.get_allocated_count(), 2);
        assert_eq!(constructed.get(), 2);
        pool.release(a);
        let c = pool.alloc();
        assert_eq!(c, vec![1]); // initializer ran again on the recycled object
        assert_eq!(constructed.get(), 2); // no new construction
        drop(b);
    }

    #[test]
    fn oversized_alloc_falls_back_without_initializer() {
        let (mut pool, constructed) = counting_pool(1);
        let a = pool.alloc();
        let b = pool.alloc(); // oversized
        assert_eq!(b, Vec::<u32>::new()); // no initializer pass
        assert_eq!(constructed.get(), 2);
        pool.release(a);
        assert_eq!(pool.get_allocated_count(), 0);
        drop(b);
    }

    #[test]
    fn release_when_pool_is_full_is_a_no_op() {
        let (mut pool, constructed) = counting_pool(1);
        let a = pool.alloc();
        assert_eq!(pool.get_allocated_count(), 1);
        pool.release(a); // allocated_count wraps to 0, object stored back
        assert_eq!(pool.get_allocated_count(), 0);
        pool.release(Vec::new()); // pool already full: no-op
        assert_eq!(pool.get_allocated_count(), 0);
        assert_eq!(constructed.get(), 1);
    }
}
