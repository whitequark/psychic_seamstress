use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, Sender};
use std::mem;

use serde;

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

impl<T> Root<T> where T: 'static {
    fn new<V>(initial: T, validator: V) -> Box<Observable<T>>
            where V: FnMut(&mut T) + 'static {
        Box::new(Root {
            value:     initial,
            validator: Box::new(validator),
            observers: Vec::new()
        })
    }
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

struct Linked<T> {
    property:  Rc<Property<T>>
}

impl<T> Linked<T> where T: 'static {
    fn new(other: Rc<Property<T>>) -> Box<Observable<T>> {
        Box::new(Linked {
            property: other
        })
    }
}

impl<T> Observable<T> for Linked<T> {
    fn read(&self, reader: &mut FnMut(&T)) {
        self.property.0.borrow().read(reader)
    }

    fn write(&mut self, writer: &mut FnMut(&mut T)) {
        self.property.0.borrow_mut().write(writer)
    }

    fn observe(&mut self, observer: Box<FnMut(&T) + 'static>) {
        self.property.0.borrow_mut().observe(observer)
    }

    fn destruct(&mut self) -> Vec<Box<FnMut(&T)>> {
        Vec::new()
    }
}

struct Derived<T, U> {
    property: Rc<Property<U>>,
    map_to:   Box<Fn(&U, T) -> U + 'static>,
    map_from: Rc<Box<Fn(&U) -> T + 'static>>
}

impl<T, U> Derived<T, U> where T: 'static, U: 'static {
    fn new<MT, MF>(other: Rc<Property<U>>, map_to: MT, map_from: MF) -> Box<Observable<T>>
            where MT: Fn(&U, T) -> U + 'static, MF: Fn(&U) -> T + 'static {
        Box::new(Derived {
            property: other.clone(),
            map_to:   Box::new(map_to),
            map_from: Rc::new(Box::new(map_from))
        })
    }
}

impl<T, U> Observable<T> for Derived<T, U> where T: 'static, U: 'static {
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
        Rc::new(Property(RefCell::new(Root::new(initial, |_| ()))))
    }

    pub fn with_validator<V>(mut initial: T, mut validator: V) -> Rc<Property<T>>
            where V: FnMut(&mut T) + 'static {
        validator(&mut initial);
        Rc::new(Property(RefCell::new(Root::new(initial, validator))))
    }

    pub fn linked(other: Rc<Property<T>>) -> Rc<Property<T>> {
        Rc::new(Property(RefCell::new(Linked::new(other))))
    }

    pub fn derived<MT, MF, U>(other: Rc<Property<U>>, map_to: MT, map_from: MF) -> Rc<Property<T>>
            where MT: Fn(&U, T) -> U + 'static, MF: Fn(&U) -> T + 'static, U: 'static {
        Rc::new(Property(RefCell::new(Derived::new(other, map_to, map_from))))
    }

    pub fn link(&self, other: Rc<Property<T>>) {
        let mut replaced = Linked::new(other);
        mem::swap(&mut *self.0.borrow_mut(), &mut replaced);

        let mut observers = replaced.destruct();
        for observer in observers.drain(..) {
            self.0.borrow_mut().observe(observer)
        }
    }

    pub fn derive<MT, MF, U>(&self, other: Rc<Property<U>>,
                             map_to: MT, map_from: MF)
            where MT: Fn(&U, T) -> U + 'static, MF: Fn(&U) -> T + 'static, U: 'static {
        let mut replaced = Derived::new(other, map_to, map_from);
        mem::swap(&mut *self.0.borrow_mut(), &mut replaced);

        let mut observers = replaced.destruct();
        for observer in observers.drain(..) {
            self.0.borrow_mut().observe(observer)
        }
    }

    pub fn read<F, R>(&self, mut reader: F) -> R where F: FnMut(&T) -> R {
        let observable = self.0.borrow();
        let mut result = None;
        observable.read(&mut |value| result = Some(reader(value)));
        result.unwrap()
    }

    pub fn write<F, R>(&self, mut writer: F) -> R where F: FnMut(&mut T) -> R {
        let mut observable = self.0.borrow_mut();
        let mut result = None;
        observable.write(&mut |value| result = Some(writer(value)));
        result.unwrap()
    }

    pub fn observe<F>(&self, observer: F)
            where F: Fn(&T) + 'static {
        let mut observable = self.0.borrow_mut();
        observable.observe(Box::new(observer))
    }

    pub fn get(&self) -> T where T: Clone {
        self.read(|value| value.clone())
    }

    pub fn set(&self, new_value: T) where T: Clone {
        self.write(move |value| *value = new_value.clone())
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
}

impl<T> Default for Property<T> where T: Default + 'static {
    fn default() -> Property<T> {
        let value = Default::default();
        Property(RefCell::new(Box::new(Root {
            value:     value,
            validator: Box::new(|_| ()),
            observers: Vec::new()
        })))
    }
}

impl<T> serde::Serialize for Property<T> where T: serde::Serialize + 'static, {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: serde::Serializer,
    {
        self.read(|value| value.serialize(serializer))
    }
}

impl<T> serde::Deserialize for Property<T> where T: serde::Deserialize + 'static {
    fn deserialize<D>(deserializer: &mut D) -> Result<Property<T>, D::Error>
        where D: serde::Deserializer,
    {
        let value = try!(serde::Deserialize::deserialize(deserializer));
        Ok(Property(RefCell::new(Box::new(Root {
            value:     value,
            validator: Box::new(|_| ()),
            observers: Vec::new()
        }))))
    }
}
