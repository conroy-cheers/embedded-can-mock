use std::{
    sync::{Arc, Mutex, Weak},
    vec::Vec,
};

use crate::{
    filter::{Filter, FilteredStatus},
    frame::MockFrame,
};
use embedded_can::Frame;

pub(crate) struct MockBus {
    interfaces: Vec<Arc<Mutex<MockInterface>>>,
}

#[derive(Debug)]
pub enum TransmitError {
    BusNotAttached,
}

#[derive(Debug)]
pub enum MockInterfaceError {
    BusAlreadyAttached,
    BusNotAttached,
}

pub(crate) struct MockInterface {
    pub(crate) filters: Vec<Filter>,
    me: Weak<Mutex<MockInterface>>,
    bus: Weak<Mutex<MockBus>>, // TODO remove arc<mutex> spam
    received_frames: Vec<MockFrame>,
}

#[derive(Clone)]
pub struct BusHandle(Arc<Mutex<MockBus>>);

#[derive(Clone)]
pub struct InterfaceHandle(Arc<Mutex<MockInterface>>);

impl MockInterface {
    fn new(filters: Vec<Filter>) -> Arc<Mutex<Self>> {
        Arc::<Mutex<Self>>::new_cyclic(|me| {
            Mutex::new(Self {
                filters,
                me: me.clone(),
                bus: Weak::new(),
                received_frames: Vec::new(),
            })
        })
    }

    fn attach_to_bus(&mut self, bus: Arc<Mutex<MockBus>>) -> Result<(), MockInterfaceError> {
        match self.bus.upgrade() {
            Some(_) => Err(MockInterfaceError::BusAlreadyAttached),
            None => {
                bus.lock()
                    .unwrap()
                    .interfaces
                    .push(self.me.upgrade().unwrap());
                self.bus = Arc::downgrade(&bus);
                Ok(())
            }
        }
    }

    /// Transmit without requiring the caller to hold a `MutexGuard`, preventing re-entrant locking.
    fn transmit_arc(me: &Arc<Mutex<Self>>, frame: MockFrame) -> Result<(), TransmitError> {
        // Grab the bus while holding the interface lock, then drop the lock before transmitting.
        let bus = {
            let me_locked = me.lock().unwrap();
            me_locked.bus.upgrade()
        };

        match bus {
            Some(bus) => {
                bus.lock().unwrap().transmit(frame);
                Ok(())
            }
            None => Err(TransmitError::BusNotAttached),
        }
    }
}

impl MockBus {
    pub(crate) fn new() -> Self {
        Self {
            interfaces: Vec::new(),
        }
    }

    fn transmit(&self, frame: MockFrame) {
        for interface in &self.interfaces {
            let mut int = interface.lock().unwrap();
            let should_receive = if int.filters.is_empty() {
                true
            } else {
                int.filters
                    .iter()
                    .any(|filter| matches!(filter.matches(frame.id()), FilteredStatus::Received))
            };

            if should_receive {
                int.received_frames.push(frame.clone());
            }
        }
    }
}

impl BusHandle {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(MockBus::new())))
    }

    pub fn add_interface(
        &self,
        filters: Vec<Filter>,
    ) -> Result<InterfaceHandle, MockInterfaceError> {
        let interface = MockInterface::new(filters);
        interface.lock().unwrap().attach_to_bus(self.0.clone())?;
        Ok(InterfaceHandle(interface))
    }

    pub fn interface_count(&self) -> usize {
        self.0.lock().unwrap().interfaces.len()
    }
}

impl InterfaceHandle {
    pub fn new_unattached(filters: Vec<Filter>) -> Self {
        Self(MockInterface::new(filters))
    }

    pub fn attach_to_bus(&self, bus: &BusHandle) -> Result<(), MockInterfaceError> {
        self.0.lock().unwrap().attach_to_bus(bus.0.clone())
    }

    pub fn transmit(&self, frame: MockFrame) -> Result<(), TransmitError> {
        MockInterface::transmit_arc(&self.0, frame)
    }

    pub fn received_frames(&self) -> Vec<MockFrame> {
        self.0.lock().unwrap().received_frames.clone()
    }
}
