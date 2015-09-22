use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, Sender};

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
            value: RefCell::new(validator(initial)),
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
        *self.value.borrow_mut() = (*self.validator)(new_value);;

        // Borrow self.observers mutably to detect recursive set().
        let new_value = self.value.borrow();
        for observer in self.observers.borrow_mut().iter() { (*observer)(&new_value) }
    }

    pub fn observe<F>(&self, observer: F)
            where F: Fn(&T) + 'static {
        observer(&self.value.borrow());
        self.observers.borrow_mut().push(Box::new(observer))
    }

    pub fn notify(&self, channel: &Sender<T>) where T: Clone + 'static {
        self.map_notify(channel, |value| value.clone())
    }

    pub fn map_notify<M, R>(&self, channel: &Sender<R>, mapper: M)
            where M: Fn(&T) -> R + 'static, R: 'static {
        let channel = channel.clone();
        self.observe(move |value| { channel.send(mapper(value)).unwrap_or(()) })
    }

    pub fn propagate(&self, other: Rc<Property<T>>) where T: Clone + 'static {
        self.map_propagate(other, |value| value.clone())
    }

    pub fn map_propagate<M, R>(&self, other: Rc<Property<R>>, mapper: M)
            where M: Fn(&T) -> R + 'static, R: 'static {
        self.observe(move |value| { other.set(mapper(value)) })
    }
}
