arch ?= x86_64
kernel := target/kernel-$(arch).bin
iso := target/rust-os-$(arch).iso

linker_script := boot/$(arch)/linker.ld
ld_mapfile := target/linker.map
grub_cfg := boot/$(arch)/grub.cfg
assembly_source_files := $(wildcard boot/$(arch)/*.asm)
assembly_object_files := $(patsubst boot/$(arch)/%.asm, target/arch/$(arch)/%.o, $(assembly_source_files))
rust_os := target/x86_64-rust_os/release/librust_os.a
userspace_src := userspace/src
ubin1 := target/x86_64-rust_os/release/boot
disk := target/disk.img

.PHONY: all clean run debug iso

all: $(kernel)

clean:
	@rm -rf target

test:
	@sed -Ei 's/^(crate-type = ).*/\1["lib"]/g' kernel/Cargo.toml
	@cargo xtest -p rust-os-runner --bin rust-os-runner
	@sed -Ei 's/^(crate-type = ).*/\1["staticlib"]/g' kernel/Cargo.toml

$(disk): $(ubin1)
	@dd if=/dev/zero of=$(disk) bs=1000 count=100000
	@mkfs.fat -F 16 $(disk)
	@mcopy -o -i target/disk.img $(ubin1) ::/boot
	@mmd -i target/disk.img /dir1
	@mmd -i target/disk.img /dir2
	@mmd -i target/disk.img /dir3
	@mmd -i target/disk.img /dir1/sub1
	@mmd -i target/disk.img /dir1/sub2
	@sh -c "echo 'hello from nikos' | mcopy -o -i target/disk.img - ::/hi.txt"
	@sh -c "echo 'this file nested af' | mcopy -o -i target/disk.img - ::/dir1/sub1/nested.txt"

run: $(iso) $(disk)
	@qemu-system-x86_64 -m size=8000 -serial stdio --no-reboot -cdrom $(iso) -drive file=$(disk),media=disk,format=raw,bus=0,unit=0 -boot d -display gtk,zoom-to-fit=on

debug: $(iso) $(disk)
	@qemu-system-x86_64 -m size=8000 -monitor stdio -d int --no-reboot -s -S -cdrom $(iso) -drive file=$(disk),media=disk,format=raw,bus=0,unit=0 -boot d -display gtk,zoom-to-fit=on

iso: $(iso)

$(iso): $(kernel) $(grub_cfg)
	@mkdir -p target/isofiles/boot/grub
	@cp $(kernel) target/isofiles/boot/kernel.bin
	@cp $(grub_cfg) target/isofiles/boot/grub
	@grub-mkrescue -o $(iso) target/isofiles # 2> /dev/null
	@rm -r target/isofiles

$(kernel): $(rust_os) $(assembly_object_files) $(linker_script)
	@mkdir -p target
	@ld -z noreloc-overflow -n -T $(linker_script) -o $(kernel) -Map=$(ld_mapfile) $(assembly_object_files) $(rust_os)

# compile assembly files
target/arch/$(arch)/%.o: boot/$(arch)/%.asm
	@mkdir -p $(shell dirname $@)
	@nasm -felf64 $< -o $@

# compile userspace programs
$(ubin1): FORCE
	@cargo rustc -p userspace --bin boot -Z build-std=core,alloc --release -- --emit=obj -C relocation-model=static -C target-feature=+crt-static

# compile rust OS
$(rust_os): FORCE
	@cargo build -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem -p rust-os --release

FORCE: ;
