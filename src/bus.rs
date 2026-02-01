use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, Weak},
    vec::Vec,
};

use crate::{
    filter::{FilterError, matches as filter_matches, validate_filters},
    frame::MockFrame,
};
use embedded_can::Frame;
use embedded_can_interface::IdMaskFilter;

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
    InvalidFilters,
}

pub(crate) struct MockInterface {
    pub(crate) filters: Vec<IdMaskFilter>,
    me: Weak<Mutex<MockInterface>>,
    bus: Weak<Mutex<MockBus>>, // TODO remove arc<mutex> spam
    received_frames: VecDeque<MockFrame>,
    condvar: Arc<Condvar>,
}

#[derive(Clone)]
pub struct BusHandle(Arc<Mutex<MockBus>>);

#[derive(Clone)]
pub struct InterfaceHandle(Arc<Mutex<MockInterface>>);

impl MockInterface {
    fn new(filters: Vec<IdMaskFilter>) -> Arc<Mutex<Self>> {
        Arc::<Mutex<Self>>::new_cyclic(|me| {
            Mutex::new(Self {
                filters,
                me: me.clone(),
                bus: Weak::new(),
                received_frames: VecDeque::new(),
                condvar: Arc::new(Condvar::new()),
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
                    .any(|filter| filter_matches(filter, frame.id()))
            };

            if should_receive {
                int.received_frames.push_back(frame.clone());
                int.condvar.notify_all();
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
        filters: Vec<IdMaskFilter>,
    ) -> Result<InterfaceHandle, MockInterfaceError> {
        validate_filters(&filters).map_err(|_| MockInterfaceError::InvalidFilters)?;
        let interface = MockInterface::new(filters);
        interface.lock().unwrap().attach_to_bus(self.0.clone())?;
        Ok(InterfaceHandle(interface))
    }

    pub fn interface_count(&self) -> usize {
        self.0.lock().unwrap().interfaces.len()
    }
}

impl Default for BusHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl InterfaceHandle {
    pub fn new_unattached(filters: Vec<IdMaskFilter>) -> Self {
        Self(MockInterface::new(filters))
    }

    pub fn attach_to_bus(&self, bus: &BusHandle) -> Result<(), MockInterfaceError> {
        self.0.lock().unwrap().attach_to_bus(bus.0.clone())
    }

    pub fn transmit(&self, frame: MockFrame) -> Result<(), TransmitError> {
        MockInterface::transmit_arc(&self.0, frame)
    }

    pub fn received_frames(&self) -> Vec<MockFrame> {
        self.0
            .lock()
            .unwrap()
            .received_frames
            .iter()
            .cloned()
            .collect()
    }

    pub fn set_filters(&self, filters: Vec<IdMaskFilter>) -> Result<(), FilterError> {
        validate_filters(&filters)?;
        let mut int = self.0.lock().unwrap();
        int.filters = filters;
        Ok(())
    }

    pub fn pop_frame(&self) -> Option<MockFrame> {
        self.0.lock().unwrap().received_frames.pop_front()
    }

    pub fn has_frames(&self) -> bool {
        !self.0.lock().unwrap().received_frames.is_empty()
    }

    pub fn wait_for_frame(&self, timeout: Option<std::time::Duration>) -> bool {
        let mut guard = self.0.lock().unwrap();
        if let Some(timeout) = timeout {
            let condvar = guard.condvar.clone();
            let (new_guard, _) = condvar
                .wait_timeout_while(guard, timeout, |int| int.received_frames.is_empty())
                .unwrap();
            guard = new_guard;
            !guard.received_frames.is_empty()
        } else {
            while guard.received_frames.is_empty() {
                let condvar = guard.condvar.clone();
                guard = condvar.wait(guard).unwrap();
            }
            true
        }
    }
}
