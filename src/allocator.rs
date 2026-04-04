use core::{alloc::GlobalAlloc, ptr::null_mut};

use bootloader::bootinfo::MemoryMap;
use linked_list_allocator::LockedHeap;
use x86_64::{PhysAddr, VirtAddr, registers::control::Cr3, structures::paging::{FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB, frame, mapper::MapToError}};

use crate::{MEMORY, allocator::{bump::{BumpAllocator, Locked}, fixed_size_block::FixedSizeBlockAllocator, linked_list::LinkedListAllocator}, memory::{self, BootInfoFrameAllocator, active_level_4_table}, println, serial_println};

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

pub mod bump;
pub mod linked_list;
pub mod fixed_size_block;

pub const HEAP_START: usize = 0x4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 Kib

pub const USER_CODE_START: u64 = 0x400000;

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

        let aligned_top = VirtAddr::new(stack_top.as_u64() & !0xF);

        aligned_top
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

        for i in 0..512 {
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
            *ptr.add(0) = 0xEB;
            *ptr.add(1) = 0xFE;
        }
    }
}

pub fn debug_walk(addr: VirtAddr, phys_mem_offset: VirtAddr) {
    let (pml4_frame, _) = Cr3::read();
    let mut table_ptr = phys_mem_offset + pml4_frame.start_address().as_u64();

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

pub fn with_memory<F, R>(f: F) -> R
where
    F: FnOnce(&mut MemoryManager) -> R
{
    let mut guard = MEMORY.lock();
    let memory = guard.as_mut().expect("MemoryManager not initialized");
    f(memory)
}
