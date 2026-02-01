//! Shared in-memory “bus” and low-level interface handles.
//!
//! [`BusHandle`] owns a shared in-memory bus. You can attach any number of interfaces (nodes) to
//! the bus via [`BusHandle::add_interface`]. Each node is represented by an [`InterfaceHandle`]
//! and has its own receive queue and acceptance filter list.
//!
//! The bus is intentionally simple:
//! - Transmit is immediate and synchronous.
//! - Frames are broadcast to every attached interface (including the transmitter).
//! - Receive queues are unbounded (in-memory).

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

/// Errors returned when transmitting a frame via an [`InterfaceHandle`].
#[derive(Debug)]
pub enum TransmitError {
    /// The interface is not attached to any bus.
    BusNotAttached,
}

/// Errors returned by bus / interface attachment operations.
#[derive(Debug)]
pub enum MockInterfaceError {
    /// The interface was already attached to a bus.
    BusAlreadyAttached,
    /// The interface is not attached to any bus.
    BusNotAttached,
    /// One or more acceptance filters failed validation.
    InvalidFilters,
}

pub(crate) struct MockInterface {
    pub(crate) filters: Vec<IdMaskFilter>,
    me: Weak<Mutex<MockInterface>>,
    bus: Weak<Mutex<MockBus>>, // TODO remove arc<mutex> spam
    received_frames: VecDeque<MockFrame>,
    condvar: Arc<Condvar>,
}

/// Handle to a shared in-memory bus.
///
/// # Example
///
/// ```
/// use embedded_can::{Frame as _, Id, StandardId};
/// use embedded_can_mock::{BusHandle, MockFrame};
///
/// let bus = BusHandle::new();
/// let iface = bus.add_interface(vec![]).unwrap();
///
/// let frame = MockFrame::new(Id::Standard(StandardId::new(0x123).unwrap()), &[0x01]).unwrap();
/// iface.transmit(frame.clone()).unwrap();
/// assert_eq!(iface.pop_frame().unwrap(), frame);
/// ```
#[derive(Clone)]
pub struct BusHandle(Arc<Mutex<MockBus>>);

/// Handle to a single mock CAN interface (node).
///
/// Each interface has:
/// - A receive queue of frames delivered by the bus.
/// - A filter list (`IdMaskFilter`), used to decide which frames are enqueued.
///
/// Use [`InterfaceHandle::transmit`] to send a frame onto the bus, and
/// [`InterfaceHandle::pop_frame`] / [`InterfaceHandle::wait_for_frame`] to receive.
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
    /// Create a new, empty bus.
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(MockBus::new())))
    }

    /// Attach a new interface to this bus.
    ///
    /// If `filters` is empty, the interface receives all frames. Otherwise it only receives frames
    /// matching at least one filter.
    pub fn add_interface(
        &self,
        filters: Vec<IdMaskFilter>,
    ) -> Result<InterfaceHandle, MockInterfaceError> {
        validate_filters(&filters).map_err(|_| MockInterfaceError::InvalidFilters)?;
        let interface = MockInterface::new(filters);
        interface.lock().unwrap().attach_to_bus(self.0.clone())?;
        Ok(InterfaceHandle(interface))
    }

    /// Number of interfaces currently attached to the bus.
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
    /// Create a new interface that is not attached to any bus yet.
    ///
    /// Use [`InterfaceHandle::attach_to_bus`] to connect it to a [`BusHandle`].
    pub fn new_unattached(filters: Vec<IdMaskFilter>) -> Self {
        Self(MockInterface::new(filters))
    }

    /// Attach this interface to `bus`.
    ///
    /// Returns [`MockInterfaceError::BusAlreadyAttached`] if the interface is already attached.
    pub fn attach_to_bus(&self, bus: &BusHandle) -> Result<(), MockInterfaceError> {
        self.0.lock().unwrap().attach_to_bus(bus.0.clone())
    }

    /// Transmit `frame` onto the bus.
    ///
    /// Frames are broadcast to all attached interfaces (including this interface) subject to the
    /// receivers’ acceptance filters.
    pub fn transmit(&self, frame: MockFrame) -> Result<(), TransmitError> {
        MockInterface::transmit_arc(&self.0, frame)
    }

    /// Return a snapshot of all currently queued received frames.
    ///
    /// This does not remove frames from the receive queue; use [`pop_frame`](Self::pop_frame) to
    /// consume frames.
    pub fn received_frames(&self) -> Vec<MockFrame> {
        self.0
            .lock()
            .unwrap()
            .received_frames
            .iter()
            .cloned()
            .collect()
    }

    /// Replace this interface’s acceptance filter list.
    ///
    /// If `filters` is empty, the interface receives all frames. Otherwise it only receives frames
    /// matching at least one filter.
    pub fn set_filters(&self, filters: Vec<IdMaskFilter>) -> Result<(), FilterError> {
        validate_filters(&filters)?;
        let mut int = self.0.lock().unwrap();
        int.filters = filters;
        Ok(())
    }

    /// Remove and return the oldest received frame, if any.
    pub fn pop_frame(&self) -> Option<MockFrame> {
        self.0.lock().unwrap().received_frames.pop_front()
    }

    /// Returns `true` if any frames are currently queued for receive.
    pub fn has_frames(&self) -> bool {
        !self.0.lock().unwrap().received_frames.is_empty()
    }

    /// Wait until at least one frame is available to receive.
    ///
    /// - `timeout: None` blocks indefinitely.
    /// - `timeout: Some(d)` waits up to `d` and returns whether a frame became available.
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
