use core::{alloc::GlobalAlloc, ptr::null_mut};

use bootloader::bootinfo::MemoryMap;
use linked_list_allocator::LockedHeap;
use x86_64::{PhysAddr, VirtAddr, structures::paging::{FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB, frame, mapper::MapToError}};

use crate::{MEMORY, allocator::{bump::{BumpAllocator, Locked}, fixed_size_block::FixedSizeBlockAllocator, linked_list::LinkedListAllocator}, memory::{self, BootInfoFrameAllocator, active_level_4_table}};

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

pub mod bump;
pub mod linked_list;
pub mod fixed_size_block;

pub const HEAP_START: usize = 0x4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 Kib

pub const USER_CODE_START: u64 = 0x400000;

pub struct MemoryManager {
    phys_mem_offset: VirtAddr,
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
        mut mapper: OffsetPageTable<'static>,
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

    pub fn alloc_user_stack(&mut self, mapper: OffsetPageTable<'static>,) -> VirtAddr {
        let stack_top = VirtAddr::new(0x0000_7FFF_FFFF_F000);
        let stack_page = Page::containing_address(stack_top - 1);

        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE;

        self.alloc_page(stack_page, flags, mapper).unwrap();

        stack_top
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

        let flags = 
            PageTableFlags::PRESENT 
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE;

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
