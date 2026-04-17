use core::{alloc::GlobalAlloc, ptr::null_mut};

use alloc::vec::Vec;
use bootloader::bootinfo::MemoryMap;
use linked_list_allocator::LockedHeap;
use x86_64::{PhysAddr, VirtAddr, registers::control::{Cr3, Cr3Flags}, structures::paging::{FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB, frame, mapper::MapToError}};
use xmas_elf::{ElfFile, program::{ProgramHeader, Type}};

use crate::{MEMORY, allocator::{bump::{BumpAllocator, Locked}, fixed_size_block::FixedSizeBlockAllocator, linked_list::LinkedListAllocator}, memory::{self, BootInfoFrameAllocator, active_level_4_table}, println, serial_println};

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

pub mod bump;
pub mod linked_list;
pub mod fixed_size_block;

pub const KERNEL_OFFSET: u64 = 0xffff_8000_0000_0000;

pub const HEAP_START: usize = 0x4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 Kib

pub const USER_CODE_START: u64 = 0x400000;

const KERNEL_PML4_START: usize = 0;
const USER_BASE: u64 = 0x1000_0000;

pub struct MemoryManager {
    pub phys_mem_offset: VirtAddr,
    mapper: OffsetPageTable<'static>,
    frame_allocator: BootInfoFrameAllocator,
}

impl MemoryManager {
    pub unsafe fn new(
        phys_mem_offset: VirtAddr,
        memory_map: &'static MemoryMap,
    ) -> Self {
        
        let mapper = unsafe { memory::init(phys_mem_offset) };
        let frame_allocator = unsafe { BootInfoFrameAllocator::init(memory_map) };

        Self {
            phys_mem_offset,
            mapper,
            frame_allocator,
        }
    }

    pub fn init_heap(
        &mut self
    ) -> Result<(), MapToError<Size4KiB>> {
        let page_range = {
            let heap_start = VirtAddr::new(HEAP_START as u64);
            let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
            let heap_start_page = Page::containing_address(heap_start);
            let heap_end_page = Page::containing_address(heap_end);
            Page::range_inclusive(heap_start_page, heap_end_page)
        };

        for page in page_range {
            let frame = self.frame_allocator
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?;
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
            unsafe {
                self.mapper.map_to(page, frame, flags, &mut self.frame_allocator)?.flush()
            };
        }

        unsafe {
            ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
        }

        Ok(())
    }

    pub fn alloc_page(
        &mut self,
        page: Page,
        flags: PageTableFlags,
        mapper: &mut OffsetPageTable<'static>,
    ) -> Result<(), MapToError<Size4KiB>> {
        let frame = self    
            .frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        unsafe {
            mapper 
                .map_to(page, frame, flags, &mut self.frame_allocator)?
                .flush();
        }
        
        Ok(())
    }

    // pub fn alloc_range(// will fix later
    //     &mut self,
    //     start: VirtAddr,
    //     size: usize,
    //     flags: PageTableFlags,
    // ) -> Result<(), MapToError<Size4KiB>> {
    //     let start_page = Page::containing_address(start);
    //     let end_page = Page::containing_address(start + size as u64 - 1);

    //     for page in Page::range_inclusive(start_page, end_page) {
    //         self.alloc_page(page, flags)?;
    //     }

    //     Ok(())
    // }

    pub fn alloc_user_stack(&mut self, pml4: PhysFrame) -> VirtAddr {
        // println!("Allocating user stack");
        let mut mapper = unsafe { self.mapper_for(pml4) };
        
        const STACK_PAGES: usize = 4;

        let stack_top = VirtAddr::new(0x0000_7FFF_FFFF_F000);
        let stack_start = stack_top - (STACK_PAGES as u64 * 4096);

        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE
            | PageTableFlags::NO_EXECUTE;

        for i in 0..STACK_PAGES {
            let addr = stack_start + (i as u64 * 4096);
            let page = Page::containing_address(addr);

            self.alloc_page(page, flags, &mut mapper).unwrap();// FIXME
        }

        // let mut rsp = stack_top -16;// stopping stuff from being written to bottom of stack

        let old = Cr3::read().0;
        unsafe { Cr3::write(pml4, Cr3Flags::empty()) };

        let rsp = unsafe { VirtAddr::new(build_user_stack(stack_top.as_u64())) };// fix me

        unsafe { Cr3::write(old, Cr3Flags::empty()) };

        rsp
    }

    pub fn new_address_space(&mut self) -> PhysFrame {
        let frame = self
            .frame_allocator
            .allocate_frame()
            .expect("no frames to allocate");

        let phys = frame.start_address();
        let virt = VirtAddr::new(phys.as_u64() + self.phys_mem_offset.as_u64());

        let pml4: &mut PageTable = unsafe { &mut *(virt.as_mut_ptr()) };
        pml4.zero();

        let current_pm14 = unsafe {active_level_4_table(self.phys_mem_offset) };

        for i in KERNEL_PML4_START..512 {
            pml4[i] = current_pm14[i].clone();// cloning kernel into new address space
        }

        frame
    }

    pub unsafe fn mapper_for(
        &self,
        frame: PhysFrame,
    ) -> OffsetPageTable<'static> {
        let virt = self.phys_to_virt(frame.start_address());
        let table: &mut PageTable = unsafe { &mut *(virt.as_mut_ptr()) };

        unsafe {
            OffsetPageTable::new(table, self.phys_mem_offset)
        }
    }

    pub fn map_user_pages(
        &mut self,
        frame: PhysFrame,
    ) -> Result<(), MapToError<Size4KiB>> {
        let mut mapper = unsafe { self.mapper_for(frame) };

        unsafe {
            mapper.map_to(
                Page::containing_address(VirtAddr::new(0x400000)), 
                self.frame_allocator.allocate_frame().unwrap(), 
                PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::USER_ACCESSIBLE, 
                &mut self.frame_allocator
            )?.flush();
        }

        Ok(())
    }

    fn phys_to_virt(&self, phys: PhysAddr) -> VirtAddr {
        VirtAddr::new(phys.as_u64() + self.phys_mem_offset.as_u64())
    }

    pub fn map_user_code(
        &mut self,
        pml4: PhysFrame,
        virt: VirtAddr,
    ) -> Result<(), MapToError<Size4KiB>> {
        let mut mapper = unsafe { self.mapper_for(pml4) };

        let page = Page::containing_address(virt);

        let frame = self
            .frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let mut flags = 
            PageTableFlags::PRESENT 
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE;

        flags.remove(PageTableFlags::NO_EXECUTE);

            unsafe {
                mapper.map_to(page, frame, flags, &mut self.frame_allocator)?.flush();
            }

        Ok(())
    }

    pub fn write_user_code(
        &mut self,
        virt: VirtAddr
    ) {
        let ptr = virt.as_mut_ptr::<u8>();

        unsafe {
            *ptr.add(0) = 0xCC;
        }
    }

    pub fn map_vga_buffer(&mut self, pml4: PhysFrame) -> Result<(), MapToError<Size4KiB>> {
        let mut mapper = unsafe { self.mapper_for(pml4) };

        let vga_virt = VirtAddr::new(KERNEL_OFFSET + 0xb8000);
        let vga_phys = PhysAddr::new(0xb8000);

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            mapper.map_to(
                Page::containing_address(vga_virt), 
                PhysFrame::containing_address(vga_phys), 
                flags, 
                &mut self.frame_allocator)?.flush();
        }

        Ok(())
    }

    pub fn load_elf(
        &mut self,
        pml4: PhysFrame,
        elf_bytes: &[u8],
    ) -> u64 {
        let mut aligned = Vec::with_capacity(elf_bytes.len());
        aligned.extend_from_slice(elf_bytes);

        let elf = ElfFile::new(aligned.as_slice()).expect("Invalid ELF");

        let min_vaddr = elf.program_iter()
            .filter_map(|ph| {
                if let Ok(Type::Load) = ph.get_type() {
                    Some(ph.virtual_addr())
                } else {
                    None
                }
            })
            .min()
            .expect("No loadable segments");

        let load_bias = USER_BASE - (min_vaddr & !0xfff);

        let entry = elf.header.pt2.entry_point() + load_bias;

        let old = Cr3::read().0;
        unsafe { Cr3::write(pml4, Cr3Flags::empty()) };

        let mut mapper = unsafe { self.mapper_for(pml4) };

        for ph in elf.program_iter() {// iterate through program headers
            if let Ok(Type::Load) = ph.get_type() {
                self.load_segment(&mut mapper, elf_bytes, ph, load_bias);
            }
        }

        unsafe { Cr3::write(old, Cr3Flags::empty()) };

        println!("ELF entry: {:#x}", elf.header.pt2.entry_point());
        println!("Load bias: {:#x}", load_bias);
        println!("Final entry: {:#x}", entry);

        entry
    }

    fn load_segment(
        &mut self,
        mapper: &mut OffsetPageTable,
        elf_bytes: &[u8],
        ph: ProgramHeader,
        load_bias: u64,
    ) {
        let virt_addr = ph.virtual_addr() + load_bias;

        let mem_size = ph.mem_size();
        let file_size = ph.file_size();
        let offset = ph.offset();

        let aligned_start = VirtAddr::new(virt_addr & !0xfff);
        let aligned_end = VirtAddr::new((virt_addr + mem_size - 1) & !0xfff);

        let start_page = Page::containing_address(aligned_start);
        let end_page = Page::containing_address(aligned_end);

        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE;

        // if ph.flags().is_write() {
        //     flags |= PageTableFlags::WRITABLE;
        // }

        if !ph.flags().is_execute() {
            flags |= PageTableFlags::NO_EXECUTE;
        }

        for page in Page::range_inclusive(start_page, end_page) {
            let addr = page.start_address().as_u64();

            if addr < USER_BASE {
                panic!("ELF tried to map below USER_BASE");
            }

            let frame = self.frame_allocator.allocate_frame().unwrap();

            unsafe {
                mapper.map_to(page, frame, flags, &mut self.frame_allocator)
                    .unwrap()
                    .flush();
            }
        }

        let data = &elf_bytes[offset as usize .. (offset + file_size) as usize];

        let page_offset = (virt_addr & 0xfff) as usize;

        let dst = (aligned_start.as_u64() + page_offset as u64) as *mut u8;

        unsafe {
            for i in 0..file_size {
                *dst.add(i as usize) = data[i as usize];
            }

            for i in file_size..mem_size {
                *dst.add(i as usize) = 0;
            }
        }
    }
}

pub fn debug_walk(addr: VirtAddr, phys_mem_offset: VirtAddr) {
    let (pml4_frame, _) = Cr3::read();
    let table_ptr = phys_mem_offset + pml4_frame.start_address().as_u64();

    let table: &PageTable = unsafe { &*(table_ptr.as_ptr()) };

    let indices = [
        addr.p4_index(),
        addr.p3_index(),
        addr.p2_index(),
        addr.p1_index(),
    ];

    let names = ["PML4", "PDPT", "PD", "PT"];

    let mut current = table;

    for (i, &index) in indices.iter().enumerate() {
        let entry = &current[index];

        println!(
            "{}[{:?}]: flags = {:?}",
            names[i],
            index,
            entry.flags()
        );

        if entry.is_unused() {
            println!("-> unused entry!");
            return;
        }

        let frame = match entry.frame() {
            Ok(f) => f,
            Err(_) => {
                println!("-> not a frame");
                return;
            }
        };

        let virt = phys_mem_offset + frame.start_address().as_u64();
        current = unsafe { &*(virt.as_ptr()) };
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

pub unsafe fn build_user_stack(stack_top: u64) -> u64 {// recreating linux abi and arguments
    let mut sp = stack_top;

    let prog = b"prog\0";

    sp -= prog.len() as u64;
    let prog_ptr = sp;

    sp &= !0xF;// realigning after writing name
    println!("SP after align: {:#x}", sp);
    println!("prog_ptr: {:#x}", prog_ptr);

    unsafe {
        core::ptr::copy_nonoverlapping(
            prog.as_ptr(), 
            prog_ptr as *mut u8, 
            prog.len(),
        );
    

        push(&mut sp, 0);// envp
        push(&mut sp, 0);// argv
        push(&mut sp, prog_ptr);// argv[0]
        push(&mut sp, 1);// argc
    }

    sp
}

unsafe fn push(sp: &mut u64, val: u64) {
    *sp -= 8;

    assert!(*sp % 8 == 0);

    unsafe {
        *(*sp as *mut u64) = val;
    }
}

pub fn with_memory<F, R>(f: F) -> R
where
    F: FnOnce(&mut MemoryManager) -> R
{
    let mut guard = MEMORY.lock();
    let memory = guard.as_mut().expect("MemoryManager not initialized");
    f(memory)
}
