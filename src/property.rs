use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, Sender};
use std::mem;

trait Observable<T> {
    fn read(&self, reader: &mut FnMut(&T));
    fn write(&mut self, writer: &mut FnMut(&mut T));
    fn observe(&mut self, observer: Box<FnMut(&T) + 'static>);
    fn destruct(&mut self) -> Vec<Box<FnMut(&T)>>;
}

struct Root<T> {
    value:     T,
    validator: Box<FnMut(&mut T)>,
    observers: Vec<Box<FnMut(&T)>>
}

impl<T> Observable<T> for Root<T> {
    fn read(&self, reader: &mut FnMut(&T)) {
        reader(&self.value)
    }

    fn write(&mut self, writer: &mut FnMut(&mut T)) {
        writer(&mut self.value);
        (*self.validator)(&mut self.value);
        for observer in self.observers.iter_mut() {
            (*observer)(&self.value)
        }
    }

    fn observe(&mut self, mut observer: Box<FnMut(&T) + 'static>) {
        observer(&self.value);
        self.observers.push(observer)
    }

    fn destruct(&mut self) -> Vec<Box<FnMut(&T)>> {
        let mut observers = Vec::new();
        mem::swap(&mut self.observers, &mut observers);
        observers
    }
}

struct Proxy<T, U> {
    property: Rc<Property<U>>,
    map_to:   Box<Fn(&U, T) -> U + 'static>,
    map_from: Rc<Box<Fn(&U) -> T + 'static>>
}

impl<T, U> Observable<T> for Proxy<T, U> where T: 'static, U: 'static {
    fn read(&self, reader: &mut FnMut(&T)) {
        let observable = self.property.0.borrow();
        observable.read(&mut |linked_value|
            reader(&(*self.map_from)(linked_value)))
    }

    fn write(&mut self, writer: &mut FnMut(&mut T)) {
        let mut observable = self.property.0.borrow_mut();
        observable.write(&mut |linked_value| {
            let mut value = (*self.map_from)(linked_value);
            writer(&mut value);
            *linked_value = (*self.map_to)(&linked_value, value)
        })
    }

    fn observe(&mut self, mut observer: Box<FnMut(&T) + 'static>) {
        let map_from = self.map_from.clone();
        let mut observable = self.property.0.borrow_mut();
        observable.observe(Box::new(move |linked_value|
            observer(&(*map_from)(linked_value))))
    }

    fn destruct(&mut self) -> Vec<Box<FnMut(&T)>> {
        Vec::new()
    }
}

pub struct Property<T>(RefCell<Box<Observable<T>>>);

impl<T> Property<T> where T: 'static {
    pub fn new(initial: T) -> Rc<Property<T>> {
        Property::with_validator(initial, |_| ())
    }

    pub fn with_validator<V>(mut initial: T, mut validator: V) -> Rc<Property<T>>
            where V: FnMut(&mut T) + 'static {
        validator(&mut initial);
        Rc::new(Property(RefCell::new(Box::new(Root {
            value:     initial,
            validator: Box::new(validator),
            observers: Vec::new()
        }))))
    }

    pub fn read<F, R>(&self, mut reader: F) -> R where F: FnMut(&T) -> R {
        let mut result = None;
        self.0.borrow().read(&mut |value| result = Some(reader(value)));
        result.unwrap()
    }

    pub fn write<F, R>(&self, mut writer: F) -> R where F: FnMut(&mut T) -> R {
        let mut result = None;
        self.0.borrow_mut().write(&mut |value| result = Some(writer(value)));
        result.unwrap()
    }

    pub fn get(&self) -> T where T: Clone {
        self.read(|value| value.clone())
    }

    pub fn set(&self, new_value: T) where T: Clone {
        self.write(move |value| *value = new_value.clone())
    }

    pub fn observe<F>(&self, observer: F)
            where F: Fn(&T) + 'static {
        self.0.borrow_mut().observe(Box::new(observer))
    }

    pub fn notify<M, R>(&self, channel: &Sender<R>, map: M)
            where M: Fn(&T) -> R + 'static, R: 'static {
        let channel = channel.clone();
        self.observe(move |value| { channel.send(map(value)).unwrap_or(()) })
    }

    pub fn propagate<M, R>(&self, other: Rc<Property<R>>, map: M)
            where M: Fn(&T) -> R + 'static, R: 'static {
        self.observe(move |value| { other.write(|other_value| *other_value = map(value)) })
    }

    pub fn link<MT, MF, U>(&self, other: Rc<Property<U>>, map_to: MT, map_from: MF)
            where MT: Fn(&U, T) -> U + 'static, MF: Fn(&U) -> T + 'static, U: 'static {
        let mut unlinked = Box::new(Proxy {
            property: other,
            map_to:   Box::new(map_to),
            map_from: Rc::new(Box::new(map_from))
        }) as Box<Observable<_>>;
        mem::swap(&mut *self.0.borrow_mut(), &mut unlinked);

        let mut observers = unlinked.destruct();
        for observer in observers.drain(..) {
            self.0.borrow_mut().observe(observer)
        }
    }
}
