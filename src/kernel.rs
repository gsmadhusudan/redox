#![feature(alloc)]
#![feature(asm)]
#![feature(box_syntax)]
#![feature(core_simd)]
#![feature(core_slice_ext)]
#![feature(core_str_ext)]
#![feature(fnbox)]
#![feature(fundamental)]
#![feature(lang_items)]
#![feature(no_std)]
#![feature(rc_unique)]
#![feature(unboxed_closures)]
#![feature(unsafe_no_drop_flag)]
#![no_std]

extern crate alloc;

#[macro_use]
extern crate mopa;

use core::fmt;
use core::mem::size_of;
use core::ptr;

use alloc::boxed::*;
use alloc::rc::*;

use common::debug::*;
use common::pio::*;
use common::memory::*;
use common::string::*;
use common::url::*;

use drivers::keyboard::*;
use drivers::mouse::*;
use drivers::pci::*;
use drivers::ps2::*;
use drivers::serial::*;

use graphics::bmp::*;

use programs::executor::*;
use programs::filemanager::*;
use programs::session::*;

use schemes::file::*;
use schemes::http::*;
use schemes::memory::*;
use schemes::pci::*;
use schemes::random::*;

mod common {
    pub mod debug;
    pub mod elf;
    pub mod memory;
    pub mod pci;
    pub mod pio;
    pub mod random;
    pub mod string;
    pub mod vec;
    pub mod url;
}

mod drivers {
    pub mod disk;
    pub mod keyboard;
    pub mod mouse;
    pub mod pci;
    pub mod ps2;
    pub mod serial;
}

mod filesystems {
    pub mod unfs;
}

mod graphics {
    pub mod bmp;
    pub mod color;
    pub mod display;
    pub mod point;
    pub mod size;
    pub mod window;
}

mod network {
    pub mod arp;
    pub mod common;
    pub mod ethernet;
    pub mod icmp;
    pub mod intel8254x;
    pub mod ipv4;
    pub mod rtl8139;
    pub mod tcp;
    pub mod udp;
}

mod programs {
    pub mod editor;
    pub mod executor;
    pub mod filemanager;
    pub mod session;
    pub mod viewer;
}

mod schemes {
    pub mod file;
    pub mod http;
    pub mod ide;
    pub mod memory;
    pub mod pci;
    pub mod random;
}

mod usb {
    pub mod xhci;
}

static mut session_ptr: *mut Session = 0 as *mut Session;

unsafe fn init(){
    serial_init();

    dd(size_of::<usize>() * 8);
    d(" bits");
    dl();

    page_init();
    cluster_init();

    session_ptr = alloc(size_of::<Session>()) as *mut Session;
    *session_ptr = Session::new();
    let session = &mut *session_ptr;

    session.items.insert(0, Rc::new(FileManager::new()));

    keyboard_init();
    mouse_init();

    session.modules.push(Rc::new(PS2));
    session.modules.push(Rc::new(Serial::new(0x3F8, 0x4)));

    pci_init(session);

    session.modules.push(Rc::new(FileScheme));
    session.modules.push(Rc::new(HTTPScheme));
    session.modules.push(Rc::new(MemoryScheme));
    session.modules.push(Rc::new(PCIScheme));
    session.modules.push(Rc::new(RandomScheme));

    session.request(&URL::from_string("file:///background.bmp".to_string()), box |response: String|{
        if response.data as usize > 0 {
            (*session_ptr).display.background = BMP::from_data(response.data as usize);
        }
    });
}

#[no_mangle]
pub unsafe fn kernel(interrupt: u32) {
    //Preserve regs for kernel calls
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;
    asm!("" : "={eax}"(eax), "={ebx}"(ebx), "={ecx}"(ecx), "={edx}"(edx) : : : "intel");

    match interrupt {
        0x20 => (), //timer
        0x21 => (*session_ptr).on_irq(0x1), //keyboard
        0x23 => (*session_ptr).on_irq(0x3), // serial 2 and 4
        0x24 => (*session_ptr).on_irq(0x4), // serial 1 and 3
        0x29 => (*session_ptr).on_irq(0x9), //pci
        0x2A => (*session_ptr).on_irq(0xA), //pci
        0x2B => (*session_ptr).on_irq(0xB), //pci
        0x2C => (*session_ptr).on_irq(0xC), //mouse
        0x2E => (*session_ptr).on_irq(0xE), //disk
        0x2F => (*session_ptr).on_irq(0xF), //disk
        0x80 => { // kernel calls
            match eax {
                0x1 => {
                    d("Session Request: ");
                    let url: &URL = &*(ebx as *const URL);
                    let callback: Box<FnBox(&mut SessionItem, String)> = ptr::read(ecx as *const Box<FnBox(&mut SessionItem, String)>);
                    unalloc(ecx as usize);
                    url.d();
                    dl();

                    let session = &mut *session_ptr;
                    if session.current_item >= 0 {
                        match session.items.get((*session).current_item as usize) {
                            Option::Some(item) => {
                                let item_copy = item.clone();
                                session.request(url, box move |response|{
                                    match Rc::unsafe_get_mut(&item_copy).downcast_mut::<Executor>(){
                                        Option::Some(item_downcast) => item_downcast.on_response(response, callback),
                                        Option::None => d("Failed to downcast\n")
                                    }
                                });
                            },
                            Option::None => d("Failed to find current item\n")
                        }
                    }
                },
                _ => {
                    d("System Call");
                    d(" EAX:");
                    dh(eax as usize);
                    d(" EBX:");
                    dh(ebx as usize);
                    d(" ECX:");
                    dh(ecx as usize);
                    d(" EDX:");
                    dh(edx as usize);
                    dl();
                }
            }
        },
        0xFF => { // main loop
            init();

            loop {
                (*session_ptr).on_poll();
                (*session_ptr).redraw();
                asm!("sti");
                asm!("hlt");
                asm!("cli"); // TODO: Allow preempting
            }
        },
        0x0 => {
            d("Divide by zero exception\n");
        },
        0x1 => {
            d("Debug exception\n");
        },
        0x2 => {
            d("Non-maskable interrupt\n");
        },
        0x3 => {
            d("Breakpoint exception\n");
        },
        0x4 => {
            d("Overflow exception\n");
        },
        0x5 => {
            d("Bound range exceeded exception\n");
        },
        0x6 => {
            d("Invalid opcode exception\n");
        },
        0x7 => {
            d("Device not available exception\n");
        },
        0x8 => {
            d("Double fault\n");
        },
        0xA => {
            d("Invalid TSS exception\n");
        },
        0xB => {
            d("Segment not present exception\n");
        },
        0xC => {
            d("Stack-segment fault\n");
        },
        0xD => {
            d("General protection fault\n");
        },
        0xE => {
            d("Page fault\n");
        },
        0x10 => {
            d("x87 floating-point exception\n");
        },
        0x11 => {
            d("Alignment check exception\n");
        },
        0x12 => {
            d("Machine check exception\n");
        },
        0x13 => {
            d("SIMD floating-point exception\n");
        },
        0x14 => {
            d("Virtualization exception\n");
        },
        0x1E => {
            d("Security exception\n");
        },
        _ => {
            d("I: ");
            dh(interrupt as usize);
            dl();
        }
    }

    if interrupt >= 0x20 && interrupt < 0x30 {
        if interrupt >= 0x28 {
            outb(0xA0, 0x20);
        }

        outb(0x20, 0x20);
    }
}

#[lang = "panic_fmt"]
pub extern fn panic_fmt(fmt: fmt::Arguments, file: &'static str, line: u32) -> ! {
    d("PANIC: ");
    d(file);
    d(": ");
    dh(line as usize);
    dl();
    unsafe{
        asm!("cli");
        asm!("hlt");
    }
    loop{}
}

#[no_mangle]
pub extern "C" fn memcmp(a: *mut u8, b: *const u8, len: isize) -> isize {
    unsafe {
        let mut i = 0;
        while i < len {
            let c_a = *a.offset(i);
            let c_b = *b.offset(i);
            if c_a != c_b{
                return c_a as isize - c_b as isize;
            }
            i += 1;
        }
        return 0;
    }
}

#[no_mangle]
pub extern "C" fn memmove(dst: *mut u8, src: *const u8, len: isize){
    unsafe {
        if src < dst {
            let mut i = len;
            while i > 0 {
                i -= 1;
                *dst.offset(i) = *src.offset(i);
            }
        }else{
            let mut i = 0;
            while i < len {
                *dst.offset(i) = *src.offset(i);
                i += 1;
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn memcpy(dst: *mut u8, src: *const u8, len: isize){
    unsafe {
        let mut i = 0;
        while i < len {
            *dst.offset(i) = *src.offset(i);
            i += 1;
        }
    }
}

#[no_mangle]
pub extern "C" fn memset(src: *mut u8, c: i32, len: isize) {
    unsafe {
        let mut i = 0;
        while i < len {
            *src.offset(i) = c as u8;
            i += 1;
        }
    }
}
