RUSTC=rustc
RUSTCFLAGS=--target=i386-elf-redox.json \
	-C no-vectorize-loops -C no-vectorize-slp -C relocation-model=static -C code-model=kernel -C no-stack-check -C opt-level=2 \
	-Z no-landing-pads \
	-A dead-code -A deprecated \
	-L.
AS=nasm
AWK=awk
CUT=cut
FIND=find
LD=ld
LDARGS=-m elf_i386
RM=rm -f
SED=sed
SORT=sort
VB=virtualbox
VBM=VBoxManage
VB_AUDIO="pulse"

ifeq ($(OS),Windows_NT)
	SHELL=windows\sh
	LD=windows/i386-elf-ld
	AS=windows/nasm
	AWK=windows/awk
	CUT=windows/cut
	FIND=windows/find
	RM=windows/rm -f
	SED=windows/sed
	SORT=windows/sort
	VB="C:/Program Files/Oracle/VirtualBox/VirtualBox"
	VBM="C:/Program Files/Oracle/VirtualBox/VBoxManage"
	VB_AUDIO="dsound"
else
	UNAME := $(shell uname)
	ifeq ($(UNAME),Darwin)
		LD=i386-elf-ld
		VB="/Applications/VirtualBox.app/Contents/MacOS/VirtualBox"
		VBM="/Applications/VirtualBox.app/Contents/MacOS/VBoxManage"
		VB_AUDIO="coreaudio"
	endif
endif

all: harddrive.bin

doc: src/kernel.rs libcore.rlib liballoc.rlib
	rustdoc --target=i386-elf-redox.json -L. $<

libcore.rlib: rust/libcore/lib.rs
	$(RUSTC) $(RUSTCFLAGS) -o $@ $<

liballoc.rlib: rust/liballoc/lib.rs libcore.rlib
	$(RUSTC) $(RUSTCFLAGS) -o $@ $<

liballoc_system.rlib: rust/liballoc_system/lib.rs libcore.rlib
	$(RUSTC) $(RUSTCFLAGS) -o $@ $<

libcollections.rlib: rust/libcollections/lib.rs libcore.rlib liballoc.rlib liballoc_system.rlib
	$(RUSTC) $(RUSTCFLAGS) -o $@ $<

kernel.rlib: src/kernel.rs libcore.rlib liballoc.rlib liballoc_system.rlib
	$(RUSTC) $(RUSTCFLAGS) -C lto -o $@ $<

kernel.bin: kernel.rlib src/kernel.ld
	$(LD) $(LDARGS) -o $@ -T src/kernel.ld $<

kernel.list: kernel.bin
	objdump -C -M intel -d $< > $@

filesystem/asm/%.bin: filesystem/asm/%.asm src/program.ld
	$(AS) -f elf -o $*.o $<
	$(LD) $(LDARGS) -o $@ -T src/program.ld $*.o

filesystem/%.bin: filesystem/%.rs src/program.rs src/program.ld libcore.rlib liballoc.rlib
	$(SED) "s|APPLICATION_PATH|$<|" src/program.rs > $*.gen
	$(RUSTC) $(RUSTCFLAGS) -C lto -o $*.rlib $*.gen
	$(LD) $(LDARGS) -o $@ -T src/program.ld $*.rlib

filesystem.gen: filesystem/httpd.bin filesystem/game.bin filesystem/terminal.bin filesystem/asm/linux.bin filesystem/asm/gpe_code.bin filesystem/asm/gpe_data.bin
	$(FIND) filesystem -not -path '*/\.*' -type f -o -type l | $(CUT) -d '/' -f2- | $(SORT) | $(AWK) '{printf("file %d,\"%s\"\n", NR, $$0)}' > $@

harddrive.bin: src/loader.asm kernel.bin filesystem.gen
	$(AS) -f bin -o $@ -isrc/ -ifilesystem/ $<

virtualbox: harddrive.bin
	echo "Delete VM"
	-$(VBM) unregistervm Redox --delete
	echo "Delete Disk"
	-$(RM) harddrive.vdi
	echo "Create VM"
	$(VBM) createvm --name Redox --register
	echo "Set Configuration"
	$(VBM) modifyvm Redox --memory 512
	$(VBM) modifyvm Redox --vram 64
	$(VBM) modifyvm Redox --nic1 nat
	$(VBM) modifyvm Redox --nictype1 82540EM
	$(VBM) modifyvm Redox --nictrace1 on
	$(VBM) modifyvm Redox --nictracefile1 network.pcap
	$(VBM) modifyvm Redox --uart1 0x3F8 4
	$(VBM) modifyvm Redox --uartmode1 file serial.log
	$(VBM) modifyvm Redox --usb on
	$(VBM) modifyvm Redox --audio $(VB_AUDIO)
	$(VBM) modifyvm Redox --audiocontroller ac97
	echo "Create Disk"
	$(VBM) convertfromraw $< harddrive.vdi
	echo "Attach Disk"
	$(VBM) storagectl Redox --name IDE --add ide --controller PIIX4 --bootable on
	$(VBM) storageattach Redox --storagectl IDE --port 0 --device 0 --type hdd --medium harddrive.vdi
	echo "Run VM"
	$(VB) --startvm Redox --dbg

qemu: harddrive.bin
	-qemu-system-i386 -net nic,model=rtl8139 -net user -net dump,file=network.pcap \
			-usb -device usb-tablet \
			-device usb-ehci,id=ehci -device nec-usb-xhci,id=xhci \
			-soundhw ac97 \
			-serial mon:stdio -d guest_errors -enable-kvm -hda $<

qemu_tap: harddrive.bin
	sudo tunctl -t tap_redox -u "${USER}"
	sudo ifconfig tap_redox 10.85.85.1 up
	-qemu-system-i386 -net nic,model=rtl8139 -net tap,ifname=tap_redox,script=no,downscript=no -net dump,file=network.pcap \
			-usb -device usb-tablet \
			-device usb-ehci,id=ehci -device nec-usb-xhci,id=xhci \
			-soundhw ac97 \
			-serial mon:stdio -d guest_errors -enable-kvm -hda $<
	sudo ifconfig tap_redox down
	sudo tunctl -d tap_redox

qemu_tap_8254x: harddrive.bin
	sudo tunctl -t tap_redox -u "${USER}"
	sudo ifconfig tap_redox 10.85.85.1 up
	-qemu-system-i386 -net nic,model=e1000 -net tap,ifname=tap_redox,script=no,downscript=no -net dump,file=network.pcap \
			-usb -device usb-tablet \
			-device usb-ehci,id=ehci -device nec-usb-xhci,id=xhci \
			-soundhw ac97 \
			-serial mon:stdio -d guest_errors -enable-kvm -hda $<
	sudo ifconfig tap_redox down
	sudo tunctl -d tap_redox

virtualbox_tap: harddrive.bin
	echo "Delete VM"
	-$(VBM) unregistervm Redox --delete
	echo "Delete Disk"
	-$(RM) harddrive.vdi
	echo "Create VM"
	$(VBM) createvm --name Redox --register
	echo "Create Bridge"
	sudo tunctl -t tap_redox -u "${USER}"
	sudo ifconfig tap_redox 10.85.85.1 up
	echo "Set Configuration"
	$(VBM) modifyvm Redox --memory 512
	$(VBM) modifyvm Redox --vram 64
	$(VBM) modifyvm Redox --nic1 bridged
	$(VBM) modifyvm Redox --nictype1 82540EM
	$(VBM) modifyvm Redox --nictrace1 on
	$(VBM) modifyvm Redox --nictracefile1 network.pcap
	$(VBM) modifyvm Redox --bridgeadapter1 tap_redox
	$(VBM) modifyvm Redox --uart1 0x3F8 4
	$(VBM) modifyvm Redox --uartmode1 file serial.log
	$(VBM) modifyvm Redox --usb on
	$(VBM) modifyvm Redox --audio $(VB_AUDIO)
	$(VBM) modifyvm Redox --audiocontroller ac97
	echo "Create Disk"
	$(VBM) convertfromraw $< harddrive.vdi
	echo "Attach Disk"
	$(VBM) storagectl Redox --name IDE --add ide --controller PIIX4 --bootable on
	$(VBM) storageattach Redox --storagectl IDE --port 0 --device 0 --type hdd --medium harddrive.vdi
	echo "Run VM"
	-$(VB) --startvm Redox --dbg
	echo "Delete Bridge"
	sudo ifconfig tap_redox down
	sudo tunctl -d tap_redox

arping:
	arping -I tap_redox 10.85.85.2

ping:
	ping 10.85.85.2

wireshark:
	wireshark network.pcap

clean:
	$(RM) -f *.bin *.gen *.list *.log *.o *.pcap *.rlib *.vdi filesystem/*.bin filesystem/asm/*.bin
