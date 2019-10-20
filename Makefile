arch ?= x86_64
kernel := target/kernel-$(arch).bin
kernel_debug := target/kernel-$(arch)-dbg.bin
iso := target/rust-os-$(arch).iso
iso_debug := target/rust-os-$(arch)-debug.iso

linker_script := boot/$(arch)/linker.ld
ld_mapfile := target/linker.map
grub_cfg := boot/$(arch)/grub.cfg
assembly_source_files := $(wildcard boot/$(arch)/*.asm)
assembly_object_files := $(patsubst boot/$(arch)/%.asm, target/arch/$(arch)/%.o, $(assembly_source_files))
rust_os := target/x86_64-rust_os/release/librust_os.a
rust_os_debug := target/x86_64-rust_os/debug/librust_os.a

.PHONY: all clean run debug iso

all: $(kernel)

clean:
	@rm -rf target

test:
	@sed -Ei 's/^(crate-type = ).*/\1["lib"]/g' kernel/Cargo.toml
	@cargo xtest -p rust-os-runner --bin rust-os-runner
	@sed -Ei 's/^(crate-type = ).*/\1["staticlib"]/g' kernel/Cargo.toml

run: $(iso)
	@qemu-system-x86_64 -m size=1000 -d int --no-reboot -cdrom $(iso)

debug: $(iso_debug)
	@qemu-system-x86_64 -m size=1000 -d int --no-reboot -s -S -cdrom $(iso_debug)

iso: $(iso)

$(iso): $(kernel) $(grub_cfg)
	@mkdir -p target/isofiles/boot/grub
	@cp $(kernel) target/isofiles/boot/kernel.bin
	@cp $(grub_cfg) target/isofiles/boot/grub
	@grub-mkrescue -o $(iso) target/isofiles 2> /dev/null
	@rm -r target/isofiles

$(iso_debug): $(kernel_debug) $(grub_cfg)
	@mkdir -p target/isofiles/boot/grub
	@cp $(kernel_debug) target/isofiles/boot/kernel.bin
	@cp $(grub_cfg) target/isofiles/boot/grub
	@grub-mkrescue -o $(iso_debug) target/isofiles 2> /dev/null
	@rm -r target/isofiles

$(kernel): $(rust_os) $(assembly_object_files) $(linker_script)
	@mkdir -p target
	@ld -z noreloc-overflow -n -T $(linker_script) -o $(kernel) -Map=$(ld_mapfile) $(assembly_object_files) $(rust_os)

$(kernel_debug): $(rust_os_debug) $(assembly_object_files) $(linker_script)
	@mkdir -p target
	@ld -z noreloc-overflow -n -T $(linker_script) -o $(kernel_debug) -Map=$(ld_mapfile) $(assembly_object_files) $(rust_os_debug)

# compile assembly files
target/arch/$(arch)/%.o: boot/$(arch)/%.asm
	@mkdir -p $(shell dirname $@)
	@nasm -felf64 $< -o $@

# compile rust OS
$(rust_os): FORCE
	@cargo xbuild -p rust-os --release

$(rust_os_debug): FORCE
	@cargo xbuild -p rust-os

FORCE: ;
