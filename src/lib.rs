//! In-memory mock for embedded CAN traits.
//! Routes frames between attached interfaces without timing or hardware.
//! Implements transmit/receive, filtering, buffering, and builder glue for tests.

pub mod bus;
pub mod filter;
pub mod frame;

pub use bus::{BusHandle, InterfaceHandle, MockInterfaceError, TransmitError};
pub use filter::FilterError;
pub use frame::MockFrame;

use embedded_can_interface::{
    BlockingControl, BufferedIo, BuilderBinding, FilterConfig, IdMaskFilter, RxFrameIo, SplitTxRx,
    TxFrameIo, TxRxState,
};
use std::time::Duration;

/// Error type for the mock backend.
#[derive(Debug)]
pub enum MockError {
    BusNotAttached,
    BusAlreadyAttached,
    Timeout,
    WouldBlock,
    InvalidFilters,
}

impl From<TransmitError> for MockError {
    fn from(err: TransmitError) -> Self {
        match err {
            TransmitError::BusNotAttached => MockError::BusNotAttached,
        }
    }
}

impl From<FilterError> for MockError {
    fn from(_err: FilterError) -> Self {
        MockError::InvalidFilters
    }
}

impl From<MockInterfaceError> for MockError {
    fn from(err: MockInterfaceError) -> Self {
        match err {
            MockInterfaceError::BusAlreadyAttached => MockError::BusAlreadyAttached,
            MockInterfaceError::BusNotAttached => MockError::BusNotAttached,
            MockInterfaceError::InvalidFilters => MockError::InvalidFilters,
        }
    }
}

/// Combined CAN interface over the mock backend.
#[derive(Clone)]
pub struct MockCan {
    iface: InterfaceHandle,
    #[allow(dead_code)]
    bus: BusHandle,
}

/// Transmit half of the mock backend.
#[derive(Clone)]
pub struct MockTx {
    iface: InterfaceHandle,
    #[allow(dead_code)]
    bus: BusHandle,
}

/// Receive half of the mock backend.
#[derive(Clone)]
pub struct MockRx {
    iface: InterfaceHandle,
    #[allow(dead_code)]
    bus: BusHandle,
}

impl MockCan {
    pub fn new_with_bus(bus: &BusHandle, filters: Vec<IdMaskFilter>) -> Result<Self, MockError> {
        Ok(Self {
            iface: bus.add_interface(filters).map_err(MockError::from)?,
            bus: bus.clone(),
        })
    }
}

impl TxFrameIo for MockCan {
    type Frame = MockFrame;
    type Error = MockError;

    fn send(&mut self, frame: &Self::Frame) -> Result<(), Self::Error> {
        self.iface.transmit(frame.clone()).map_err(MockError::from)
    }

    fn try_send(&mut self, frame: &Self::Frame) -> Result<(), Self::Error> {
        self.iface.transmit(frame.clone()).map_err(MockError::from)
    }

    fn send_timeout(&mut self, frame: &Self::Frame, _timeout: Duration) -> Result<(), Self::Error> {
        // Mock send is immediate.
        self.send(frame)
    }
}

impl RxFrameIo for MockCan {
    type Frame = MockFrame;
    type Error = MockError;

    fn recv(&mut self) -> Result<Self::Frame, Self::Error> {
        if let Some(frame) = self.iface.pop_frame() {
            return Ok(frame);
        }
        let has = self.iface.wait_for_frame(None);
        if has {
            self.iface.pop_frame().ok_or(MockError::Timeout)
        } else {
            Err(MockError::Timeout)
        }
    }

    fn try_recv(&mut self) -> Result<Self::Frame, Self::Error> {
        self.iface.pop_frame().ok_or(MockError::WouldBlock)
    }

    fn recv_timeout(&mut self, timeout: Duration) -> Result<Self::Frame, Self::Error> {
        if let Some(frame) = self.iface.pop_frame() {
            return Ok(frame);
        }
        let has = self.iface.wait_for_frame(Some(timeout));
        if has {
            self.iface.pop_frame().ok_or(MockError::Timeout)
        } else {
            Err(MockError::Timeout)
        }
    }

    fn wait_not_empty(&mut self) -> Result<(), Self::Error> {
        if self.iface.has_frames() {
            return Ok(());
        }
        let _ = self.iface.wait_for_frame(None);
        Ok(())
    }
}

impl SplitTxRx for MockCan {
    type Tx = MockTx;
    type Rx = MockRx;

    fn split(self) -> (Self::Tx, Self::Rx) {
        (
            MockTx {
                iface: self.iface.clone(),
                bus: self.bus.clone(),
            },
            MockRx {
                iface: self.iface,
                bus: self.bus,
            },
        )
    }
}

impl TxFrameIo for MockTx {
    type Frame = MockFrame;
    type Error = MockError;

    fn send(&mut self, frame: &Self::Frame) -> Result<(), Self::Error> {
        self.iface.transmit(frame.clone()).map_err(MockError::from)
    }

    fn try_send(&mut self, frame: &Self::Frame) -> Result<(), Self::Error> {
        self.iface.transmit(frame.clone()).map_err(MockError::from)
    }

    fn send_timeout(&mut self, frame: &Self::Frame, _timeout: Duration) -> Result<(), Self::Error> {
        self.send(frame)
    }
}

impl RxFrameIo for MockRx {
    type Frame = MockFrame;
    type Error = MockError;

    fn recv(&mut self) -> Result<Self::Frame, Self::Error> {
        if let Some(frame) = self.iface.pop_frame() {
            Ok(frame)
        } else {
            self.iface.wait_for_frame(None);
            self.iface.pop_frame().ok_or(MockError::Timeout)
        }
    }

    fn try_recv(&mut self) -> Result<Self::Frame, Self::Error> {
        self.iface.pop_frame().ok_or(MockError::WouldBlock)
    }

    fn recv_timeout(&mut self, timeout: Duration) -> Result<Self::Frame, Self::Error> {
        if let Some(frame) = self.iface.pop_frame() {
            return Ok(frame);
        }
        let has = self.iface.wait_for_frame(Some(timeout));
        if has {
            self.iface.pop_frame().ok_or(MockError::Timeout)
        } else {
            Err(MockError::Timeout)
        }
    }

    fn wait_not_empty(&mut self) -> Result<(), Self::Error> {
        if self.iface.has_frames() {
            return Ok(());
        }
        let _ = self.iface.wait_for_frame(None);
        Ok(())
    }
}

impl FilterConfig for MockCan {
    type FiltersHandle<'a> = ();
    type Error = MockError;

    fn set_filters(&mut self, filters: &[IdMaskFilter]) -> Result<(), Self::Error> {
        self.iface
            .set_filters(filters.to_vec())
            .map_err(MockError::from)
    }

    fn modify_filters(&mut self) -> Self::FiltersHandle<'_> {
        ()
    }
}

impl TxRxState for MockCan {
    type Error = MockError;

    fn is_transmitter_idle(&self) -> Result<bool, Self::Error> {
        // Mock transmit is immediate.
        Ok(true)
    }
}

impl BlockingControl for MockCan {
    type Error = MockError;

    fn set_nonblocking(&mut self, _on: bool) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub struct MockBuffered<'a, const TX: usize, const RX: usize> {
    #[allow(dead_code)]
    iface: InterfaceHandle,
    #[allow(dead_code)]
    tx: &'a mut [MockFrame; TX],
    #[allow(dead_code)]
    rx: &'a mut [MockFrame; RX],
}

impl BufferedIo for MockCan {
    type Frame = MockFrame;
    type Error = MockError;
    type Buffered<'a, const TX: usize, const RX: usize> = MockBuffered<'a, TX, RX>;

    fn buffered<'a, const TX: usize, const RX: usize>(
        &'a mut self,
        tx: &'a mut [Self::Frame; TX],
        rx: &'a mut [Self::Frame; RX],
    ) -> Self::Buffered<'a, TX, RX> {
        MockBuffered {
            iface: self.iface.clone(),
            tx,
            rx,
        }
    }
}

pub struct MockBuilder {
    bus: BusHandle,
    filters: Vec<IdMaskFilter>,
}

impl BuilderBinding for MockCan {
    type Error = MockError;
    type Builder = MockBuilder;

    fn open(_name: &str) -> Result<Self, Self::Error> {
        let bus = BusHandle::new();
        MockCan::new_with_bus(&bus, vec![])
    }

    fn builder() -> Self::Builder {
        MockBuilder {
            bus: BusHandle::new(),
            filters: Vec::new(),
        }
    }
}

impl MockBuilder {
    pub fn with_filters(mut self, filters: Vec<IdMaskFilter>) -> Result<Self, MockError> {
        self.filters = filters;
        Ok(self)
    }

    pub fn build(self) -> Result<MockCan, MockError> {
        MockCan::new_with_bus(&self.bus, self.filters)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_can::{ExtendedId, Frame as _, Id, StandardId};
    use embedded_can_interface::{
        BlockingControl, BuilderBinding, BufferedIo, FilterConfig, Id as IfaceId, IdMask,
        IdMaskFilter, RxFrameIo, SplitTxRx, TxFrameIo, TxRxState,
    };
    use std::time::Duration;

    fn standard_frame(id: u16, data: &[u8]) -> MockFrame {
        MockFrame::new(Id::Standard(StandardId::new(id).unwrap()), data).unwrap()
    }

    fn extended_frame(id: u32, data: &[u8]) -> MockFrame {
        MockFrame::new(Id::Extended(ExtendedId::new(id).unwrap()), data).unwrap()
    }

    #[test]
    fn transmit_returns_error_when_not_attached_to_bus() {
        let frame = standard_frame(0x100, &[0x10]);
        let node = InterfaceHandle::new_unattached(vec![]);

        let result = node.transmit(frame);

        assert!(matches!(result, Err(TransmitError::BusNotAttached)));
        assert!(node.received_frames().is_empty());
    }

    #[test]
    fn interface_only_attaches_once() {
        let bus = BusHandle::new();
        let node = InterfaceHandle::new_unattached(vec![]);

        node.attach_to_bus(&bus).unwrap();
        let second_attach = node.attach_to_bus(&bus);

        assert!(matches!(
            second_attach,
            Err(MockInterfaceError::BusAlreadyAttached)
        ));
        assert_eq!(bus.interface_count(), 1);
    }

    #[test]
    fn transmit_delivers_frames_to_all_attached_interfaces() {
        let frame = standard_frame(0x100, &[0x10, 0x20]);

        let bus_ref = BusHandle::new();

        let node1 = bus_ref.add_interface(vec![]).unwrap();
        let node2 = bus_ref.add_interface(vec![]).unwrap();

        node1.transmit(frame.clone()).unwrap();
        node2.transmit(frame.clone()).unwrap();

        assert_eq!(node1.received_frames().len(), 2);
        assert_eq!(node2.received_frames().len(), 2);
        assert_eq!(node1.received_frames()[0], frame);
        assert_eq!(node2.received_frames()[1], frame);
    }

    #[test]
    fn transmit_arc_forwards_frames_via_trait_extension() {
        let frame = extended_frame(0x1ABCDE0, &[0xAB, 0xCD]);
        let bus = BusHandle::new();
        let node = bus.add_interface(vec![]).unwrap();
        node.transmit(frame.clone()).unwrap();

        assert_eq!(node.received_frames(), vec![frame]);
    }

    #[test]
    fn standard_filters_gate_delivery_to_matching_frames() {
        let matching = standard_frame(0x123, &[0xAA]);
        let rejected = standard_frame(0x321, &[0xBB]);

        let bus = BusHandle::new();

        let filtered_node = bus
            .add_interface(vec![IdMaskFilter {
                id: IfaceId::Standard(StandardId::new(0x123).unwrap()),
                mask: IdMask::Standard(0x7FF),
            }])
            .unwrap();
        let unfiltered_node = bus.add_interface(vec![]).unwrap();

        unfiltered_node.transmit(matching.clone()).unwrap();
        unfiltered_node.transmit(rejected.clone()).unwrap();

        let filtered_received = filtered_node.received_frames();
        let unfiltered_received = unfiltered_node.received_frames();

        assert_eq!(unfiltered_received.len(), 2);
        assert_eq!(filtered_received, vec![matching.clone()]);
        assert!(!filtered_received.contains(&rejected));
    }

    #[test]
    fn extended_filters_gate_delivery_to_matching_frames() {
        let matching = extended_frame(0x1ABCDE01, &[0xDE, 0xAD]);
        let rejected = extended_frame(0x1ABCD000, &[0xBE, 0xEF]);

        let bus = BusHandle::new();

        let filtered_node = bus
            .add_interface(vec![IdMaskFilter {
                id: IfaceId::Extended(ExtendedId::new(0x1ABCDE00).unwrap()),
                mask: IdMask::Extended(0x1FFFFF00),
            }])
            .unwrap();
        let unfiltered_node = bus.add_interface(vec![]).unwrap();

        unfiltered_node.transmit(matching.clone()).unwrap();
        unfiltered_node.transmit(rejected.clone()).unwrap();

        let filtered_received = filtered_node.received_frames();
        let unfiltered_received = unfiltered_node.received_frames();

        assert_eq!(unfiltered_received.len(), 2);
        assert_eq!(filtered_received, vec![matching.clone()]);
        assert!(!filtered_received.contains(&rejected));
    }

    #[test]
    fn tx_frame_io_paths_deliver_frames() {
        let bus = BusHandle::new();
        let mut sender = MockCan::new_with_bus(&bus, vec![]).unwrap();
        let mut receiver = MockCan::new_with_bus(&bus, vec![]).unwrap();

        let frame1 = standard_frame(0x555, &[0x01, 0x02]);
        let frame2 = extended_frame(0x1ABCDE0, &[0xAA]);
        let frame3 = standard_frame(0x123, &[0xCC]);

        TxFrameIo::send(&mut sender, &frame1).unwrap();
        TxFrameIo::send_timeout(&mut sender, &frame2, Duration::from_millis(5)).unwrap();
        TxFrameIo::try_send(&mut sender, &frame3).unwrap();

        assert_eq!(RxFrameIo::recv(&mut receiver).unwrap(), frame1);
        assert_eq!(RxFrameIo::recv_timeout(&mut receiver, Duration::from_millis(5)).unwrap(), frame2);
        assert_eq!(RxFrameIo::recv(&mut receiver).unwrap(), frame3);
    }

    #[test]
    fn rx_frame_io_reports_errors_when_empty() {
        let bus = BusHandle::new();
        let mut node = MockCan::new_with_bus(&bus, vec![]).unwrap();

        let would_block = RxFrameIo::try_recv(&mut node);
        assert!(matches!(would_block, Err(MockError::WouldBlock)));

        let timeout = RxFrameIo::recv_timeout(&mut node, Duration::from_millis(1));
        assert!(matches!(timeout, Err(MockError::Timeout)));

        let ready_frame = standard_frame(0x321, &[0x11]);
        TxFrameIo::send(&mut node, &ready_frame).unwrap();
        RxFrameIo::wait_not_empty(&mut node).unwrap();
        assert_eq!(RxFrameIo::recv(&mut node).unwrap(), ready_frame);
    }

    #[test]
    fn split_tx_rx_halves_interoperate() {
        let bus = BusHandle::new();
        let combined = MockCan::new_with_bus(&bus, vec![]).unwrap();
        let (mut tx, mut rx) = SplitTxRx::split(combined);

        let frame = standard_frame(0x42, &[0xAA, 0xBB]);
        TxFrameIo::send(&mut tx, &frame).unwrap();
        assert_eq!(RxFrameIo::recv(&mut rx).unwrap(), frame);

        let frame2 = standard_frame(0x43, &[0xCC]);
        TxFrameIo::try_send(&mut tx, &frame2).unwrap();
        RxFrameIo::wait_not_empty(&mut rx).unwrap();
        assert_eq!(RxFrameIo::recv_timeout(&mut rx, Duration::from_millis(5)).unwrap(), frame2);
    }

    #[test]
    fn filter_config_updates_and_validates() {
        let bus = BusHandle::new();
        let mut filtered = MockCan::new_with_bus(&bus, vec![]).unwrap();
        let mut sender = MockCan::new_with_bus(&bus, vec![]).unwrap();

        let accept_only = IdMaskFilter {
            id: IfaceId::Standard(StandardId::new(0x100).unwrap()),
            mask: IdMask::Standard(0x7FF),
        };
        FilterConfig::set_filters(&mut filtered, &[accept_only]).unwrap();

        let matching = standard_frame(0x100, &[0x01]);
        let rejected = standard_frame(0x101, &[0x02]);

        TxFrameIo::send(&mut sender, &matching).unwrap();
        TxFrameIo::send(&mut sender, &rejected).unwrap();

        assert_eq!(RxFrameIo::recv(&mut filtered).unwrap(), matching);
        assert!(matches!(
            RxFrameIo::try_recv(&mut filtered),
            Err(MockError::WouldBlock)
        ));

        let invalid_filter = IdMaskFilter {
            id: IfaceId::Extended(ExtendedId::new(0x1ABCDE0).unwrap()),
            mask: IdMask::Standard(0x7FF),
        };
        let result = FilterConfig::set_filters(&mut filtered, &[invalid_filter]);
        assert!(matches!(result, Err(MockError::InvalidFilters)));
    }

    #[test]
    fn tx_rx_state_and_blocking_control_paths_work() {
        let bus = BusHandle::new();
        let mut node = MockCan::new_with_bus(&bus, vec![]).unwrap();

        assert!(TxRxState::is_transmitter_idle(&node).unwrap());
        BlockingControl::set_nonblocking(&mut node, true).unwrap();

        let frame = standard_frame(0x222, &[0x0A]);
        TxFrameIo::send(&mut node, &frame).unwrap();
        assert!(TxRxState::is_transmitter_idle(&node).unwrap());

        BlockingControl::set_nonblocking(&mut node, false).unwrap();
        assert_eq!(RxFrameIo::recv(&mut node).unwrap(), frame);
    }

    #[test]
    fn buffered_io_creates_wrapper() {
        let bus = BusHandle::new();
        let mut node = MockCan::new_with_bus(&bus, vec![]).unwrap();

        let mut tx_buf: [MockFrame; 2] =
            std::array::from_fn(|i| standard_frame(0x300 + i as u16, &[]));
        let mut rx_buf: [MockFrame; 2] =
            std::array::from_fn(|i| standard_frame(0x400 + i as u16, &[]));

        let wrapped: <MockCan as BufferedIo>::Buffered<'_, 2, 2> =
            BufferedIo::buffered(&mut node, &mut tx_buf, &mut rx_buf);
        drop(wrapped);

        assert_eq!(tx_buf.len(), 2);
        assert_eq!(rx_buf.len(), 2);
    }

    #[test]
    fn builder_binding_constructs_interfaces() {
        let mut opened = MockCan::open("loopback").unwrap();
        let loopback_frame = standard_frame(0x55, &[0xAA]);
        TxFrameIo::send(&mut opened, &loopback_frame).unwrap();
        assert_eq!(RxFrameIo::recv(&mut opened).unwrap(), loopback_frame);

        let filter = IdMaskFilter {
            id: IfaceId::Standard(StandardId::new(0x700).unwrap()),
            mask: IdMask::Standard(0x7FF),
        };
        let mut built = MockCan::builder()
            .with_filters(vec![filter])
            .unwrap()
            .build()
            .unwrap();

        let matching = standard_frame(0x700, &[0x01, 0x02]);
        let rejected = standard_frame(0x701, &[0x03]);
        TxFrameIo::send(&mut built, &matching).unwrap();
        assert_eq!(RxFrameIo::recv(&mut built).unwrap(), matching);

        TxFrameIo::send(&mut built, &rejected).unwrap();
        assert!(matches!(
            RxFrameIo::try_recv(&mut built),
            Err(MockError::WouldBlock)
        ));
    }
}
