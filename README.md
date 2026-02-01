# embedded-can-mock

Mock CAN bus + interfaces for tests and simulations.

This crate provides in-memory CAN primitives that implement the traits from
`embedded-can-interface`, so that protocol layers (ISO-TP, UDS, application buses, …) can be exercised
without real hardware.
