# TODO: make generic. if you're running this on a different computer or project, sorry, you'll have to swap out things like the /dev/ parameters and crate names
# also if you change memory.x, you might need to run cargo clean
cargo build --release --target thumbv7em-none-eabi
arm-none-eabi-objcopy -O ihex ../../target/thumbv7em-none-eabi/release/watch_firmware ../../target/thumbv7em-none-eabi/release/watch_firmware.hex
adafruit-nrfutil dfu genpkg --dev-type 0x0052 --sd-req 0x0123 --application ../../target/thumbv7em-none-eabi/release/watch_firmware.hex ../../target/thumbv7em-none-eabi/release/watch_firmware.zip
adafruit-nrfutil --verbose dfu serial -pkg ../../target/thumbv7em-none-eabi/release/watch_firmware.zip -p /dev/tty.usbmodem1101  --singlebank