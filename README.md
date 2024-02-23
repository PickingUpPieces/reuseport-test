# REUSPORT UDP load balancing test
This small program, written in Rust, tests the implemented load balancing of SO_REUSEPORT with UDP. 
The load balancing is implemented in the Linux kernel and balances according to the 4-tuple (send port+IP, recv port+IP) between the sockets.

For more information:
- https://lwn.net/Articles/542629/
- https://pavel.network/rocky-road-towards-ultimate-udp-load-balancing-server-on-linux/
- https://blog.flipkart.tech/linux-tcp-so-reuseport-usage-and-implementation-6bfbf642885a
