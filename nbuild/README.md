# nbuild - the Neotron OS Build System

Building Neotron OS involves:

* Compiling the Neotron OS source code
* Compiling and linking multiple Neotron OS utilities
* Assembling the utilities into a ROMFS image
* Linking the Neotron OS object code, including the ROMFS image

This utility automates that process.

Run `cargo nbuild --help` from the top level of the checkout for more information.
