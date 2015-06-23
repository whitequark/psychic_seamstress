use std::cell::RefCell;
use std::rc::Rc;
// use std::sync::mpsc::{sync_channel, Sender, Receiver, TrySendError};

pub struct Property<T> {
    value: RefCell<T>,
    validator: Box<Fn(T) -> T>,
    observers: RefCell<Vec<Box<Fn(&T)>>>
}

impl<T> Property<T> {
    pub fn new(initial: T) -> Rc<Property<T>> {
        Rc::new(Property {
            value: RefCell::new(initial),
            validator: Box::new(|x| { x }),
            observers: RefCell::new(Vec::new())
        })
    }

    pub fn new_validated<V>(initial: T, validator: V) -> Rc<Property<T>>
            where V: Fn(T) -> T + 'static {
        Rc::new(Property {
            value: RefCell::new(initial),
            validator: Box::new(validator),
            observers: RefCell::new(Vec::new())
        })
    }

    pub fn get(&self) -> T where T: Clone {
        self.value.borrow().clone()
    }

    pub fn with<F, Ret>(&self, mut f: F) -> Ret where F: FnMut(&T)->Ret {
        f(&self.value.borrow())
    }

    pub fn set(&self, new_value: T) {
        *self.value.borrow_mut() = (*self.validator)(new_value);
        for observer in self.observers.borrow().iter() {
            (*observer)(&self.value.borrow())
        }
    }

    pub fn observe<F>(&self, observer: F) where F: Fn(&T) + 'static {
        observer(&self.value.borrow());
        self.observers.borrow_mut().push(Box::new(observer))
    }

    // pub fn channel(&self) -> Receiver<T> where T: Clone + 'static {
    //     let (tx, rx) = sync_channel(0);
    //     self.observe(|value| {
    //         match tx.try_send(value.clone()) {
    //             Ok(_) | Err(TrySendError::Full(_)) => (),
    //             Err(TrySendError::Disconnected(_)) => () /* remove observer */
    //         }
    //     });
    //     rx
    // }
}
