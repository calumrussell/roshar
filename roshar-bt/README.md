
TODO:

- [ ] Performance Measurement

- [x] Integrate lot size

- [ ] Multiple assets

- [x] Implement trading strategy in examples

- [ ] Add perf tests

- [x] Cancel orders

- [x] Position tracking

- [x] impl get_orders

- [x] Move fill to l2

- [x] Refactor the FillModel trait, is too big, there is some overlap with order management here

- [x] Basic Config struct

- [x] Market fills are handling qty updates differently to Limit with introduction of `filled_qty` property to `L2Order`

- [ ] Add latency model

- [ ] Add cost model

- [x] Fix rounding errors - tick->price conversion is broken, also qtys stored in orderbook are suspicious should probably perform these conversions at orderbook level (but will also need to match in fill, which should be okay)

- [x] Refactor the BT creation so there is less boilerplate