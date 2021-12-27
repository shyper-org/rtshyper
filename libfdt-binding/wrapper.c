#include "wrapper.h"
#include "libfdt.h"

#define PAGE_SIZE 4096

int fdt_remove_node(void *fdt, const char *path) {
  int node = fdt_path_offset(fdt, path);
  if (node < 0) {
    return -1;
  }
  fdt_del_node(fdt, node);
  return 0;
}

int fdt_disable_node(void *fdt, const char *path) {
  int len = 0;
  int node = fdt_path_offset(fdt, path);
  if (node < 0) {
    return -1;
  }
  const char *prop = fdt_getprop(fdt, node, "status", &len);
  if (prop == NULL) {
    fdt_setprop_string(fdt, node, "status", "disabled");
    return 0;
  }
  fdt_setprop_inplace(fdt, node, "status", "NILL", len);
  return 0;
}

void fdt_add_virtio(void *fdt, const char *name, uint32_t spi_irq,
                    uint64_t address) {
  int root = fdt_path_offset(fdt, "/");
  int node = fdt_add_subnode(fdt, root, name);
  fdt_setprop(fdt, node, "dma-coherent", NULL, 0);
  fdt_setprop_string(fdt, node, "compatible", "virtio,mmio");
  fdt32_t irq[3] = {
      cpu_to_fdt32(0),
      cpu_to_fdt32(spi_irq),
      cpu_to_fdt32(0x1),
  };
  fdt_setprop(fdt, node, "interrupts", irq, sizeof(irq));
  fdt64_t addr[2] = {
      cpu_to_fdt64(address),
      cpu_to_fdt64(0x400),
  };
  fdt_setprop(fdt, node, "reg", addr, sizeof(addr));
}

void fdt_add_vm_service(void *fdt, uint32_t spi_irq, uint64_t address,
                        uint64_t len) {
  int root = fdt_path_offset(fdt, "/");
  int node = fdt_add_subnode(fdt, root, "vm_service");
  fdt_setprop_string(fdt, node, "compatible", "shyper");
  fdt32_t irq[3] = {
      cpu_to_fdt32(0),
      cpu_to_fdt32(spi_irq),
      cpu_to_fdt32(0x1),
  };
  fdt_setprop(fdt, node, "interrupts", irq, sizeof(irq));
  if (address != 0 && len != 0) {
    fdt64_t addr[2] = {
        cpu_to_fdt64(address),
        cpu_to_fdt64(len),
    };
    fdt_setprop(fdt, node, "reg", addr, sizeof(addr));
  }
}

void fdt_add_timer(void *fdt, uint32_t trigger_lvl) {
  int root = fdt_path_offset(fdt, "/");
  int node = fdt_add_subnode(fdt, root, "timer");
  fdt_setprop_string(fdt, node, "compatible", "arm,armv8-timer");
  fdt32_t irq[12] = {
      cpu_to_fdt32(0x1), cpu_to_fdt32(0xd), cpu_to_fdt32(trigger_lvl),
      cpu_to_fdt32(0x1), cpu_to_fdt32(0xe), cpu_to_fdt32(trigger_lvl),
      cpu_to_fdt32(0x1), cpu_to_fdt32(0xb), cpu_to_fdt32(trigger_lvl),
      cpu_to_fdt32(0x1), cpu_to_fdt32(0xa), cpu_to_fdt32(trigger_lvl),
  };
  fdt_setprop(fdt, node, "interrupts", irq, sizeof(irq));
}

void fdt_add_vm_service_blk(void *fdt, uint32_t spi_irq) {
  int root = fdt_path_offset(fdt, "/");
  int node = fdt_add_subnode(fdt, root, "vm_service_blk");
  fdt_setprop_string(fdt, node, "compatible", "shyper_blk");
  fdt32_t irq[3] = {
      cpu_to_fdt32(0),
      cpu_to_fdt32(spi_irq),
      cpu_to_fdt32(0x1),
  };
  fdt_setprop(fdt, node, "interrupts", irq, sizeof(irq));
}

void fdt_add_cpu(void *fdt, uint64_t linear_id, uint8_t core_id,
                 uint8_t cluster_id, const char *compatible) {
  // NOTE: this function assumes cpu-map does NOT exist and #address-cells = <2>
  char node_name[32] = "cpu@x";
  node_name[4] = linear_id + '0';
  int cpus = fdt_path_offset(fdt, "/cpus");
  int node = fdt_add_subnode(fdt, cpus, node_name);
  fdt_setprop_string(fdt, node, "compatible", compatible);
  fdt_setprop_string(fdt, node, "device_type", "cpu");
  fdt_setprop_string(fdt, node, "enable-method", "psci");
  fdt32_t reg[2] = {
      cpu_to_fdt32(0),
      cpu_to_fdt32(cluster_id << 8 | core_id),
  };
  fdt_setprop(fdt, node, "reg", reg, sizeof(reg));
}

void fdt_set_bootcmd(void *fdt, const char *cmdline) {
  int node;
  node = fdt_path_offset(fdt, "/chosen");
  fdt_setprop_string(fdt, node, "bootargs", cmdline);
}

void fdt_set_initrd(void *fdt, uint32_t start, uint32_t end) {
  // NOTE: linux,initrd-start/end only has one cell (uint32_t)
  int node;
  node = fdt_path_offset(fdt, "/chosen");
  fdt32_t addr = cpu_to_fdt32((uint32_t)start);
  fdt_setprop(fdt, node, "linux,initrd-start", &addr, sizeof(fdt32_t));
  addr = cpu_to_fdt32(end);
  fdt_setprop(fdt, node, "linux,initrd-end", &addr, sizeof(fdt32_t));
}

void fdt_set_memory(void *fdt, uint64_t region_num,
                    const struct region *regions, const char *node_name) {
  // NOTE: this function dose NOT assume memory_node existed
  int r;
#define FDT_MEMORY_REGION_MAX 4
  if (region_num == 0) {
    return;
  }
  if (region_num > FDT_MEMORY_REGION_MAX) {
    region_num = FDT_MEMORY_REGION_MAX;
  }
  int existed = fdt_node_offset_by_prop_value(fdt, 0, "device_type", "memory",
                                              (int)strlen("memory") + 1);
  if (existed > 0) {
    fdt_del_node(fdt, existed);
  }

  int node;
  int root = fdt_path_offset(fdt, "/");
  node = fdt_add_subnode(fdt, root, node_name);
  if (node < 0) {
    return;
  }
  r = fdt_setprop_string(fdt, node, "device_type", "memory");
  if (r < 0) {
    return;
  }

  fdt64_t addr[FDT_MEMORY_REGION_MAX * 2];
  for (uint64_t i = 0; i < region_num; ++i) {
    addr[2 * i] = cpu_to_fdt64(regions[i].ipa_start);
    addr[2 * i + 1] = cpu_to_fdt64(regions[i].length);
  }
  r = fdt_setprop(fdt, node, "reg", addr,
                  (int)region_num * 2 * (int)sizeof(fdt64_t));
  if (r < 0) {
    return;
  }
}

void fdt_clear_initrd(void *fdt) {
  int node;
  node = fdt_path_offset(fdt, "/chosen");
  if (node < 0) {
    return;
  }
  fdt_delprop(fdt, node, "linux,initrd-start");
  fdt_delprop(fdt, node, "linux,initrd-end");
}

void fdt_setup_gic(void *fdt, uint64_t gicd_addr, uint64_t gicc_addr,
                   const char *node_name) {
  int r;
  int node;
  node = fdt_node_offset_by_compatible(fdt, 0, "arm,cortex-a15-gic");
  if (node < 0) {
    node = fdt_node_offset_by_compatible(fdt, 0, "arm,gic-400");
    if (node < 0) {
      return;
    }
  }
  fdt64_t addr[4] = {
      cpu_to_fdt64(gicd_addr),
      cpu_to_fdt64(0x1000),
      cpu_to_fdt64(gicc_addr),
      cpu_to_fdt64(0x2000),
  };
  r = fdt_setprop(fdt, node, "reg", addr, sizeof(addr));
  fdt_nop_property(fdt, node, "interrupts");
  if (r < 0) {
    return;
  }
  r = fdt_set_name(fdt, node, node_name);
}

void fdt_setup_serial(void *fdt, const char *compatible, uint64_t addr,
                      uint32_t spi_irq) {
  int r;
  int node;
  node = fdt_node_offset_by_compatible(fdt, 0, compatible);
  if (node < 0) {
    return;
  }
  fdt64_t reg[2] = {
      cpu_to_fdt64(addr),
      cpu_to_fdt64(0x1000),
  };
  r = fdt_setprop(fdt, node, "reg", reg, sizeof(reg));
  if (r < 0) {
    return;
  }
  fdt32_t irq[3] = {
      cpu_to_fdt32(0),
      cpu_to_fdt32(spi_irq),
      cpu_to_fdt32(0x4),
  };
  r = fdt_setprop(fdt, node, "interrupts", irq, sizeof(irq));
  if (r < 0) {
    return;
  }
  r = fdt_setprop_string(fdt, node, "status", "okay");
  if (r < 0) {
    return;
  }
  r = fdt_set_name(fdt, node, "serial@0");
}

void fdt_set_stdout_path(void *fdt, const char *p) {
  int r;
  int node;
  node = fdt_path_offset(fdt, "/chosen");
  if (node < 0) {
    return;
  }
  r = fdt_setprop_string(fdt, node, "stdout-path", p);
}

void fdt_clear_stdout_path(void *fdt) {
  int node;
  node = fdt_path_offset(fdt, "/chosen");
  fdt_delprop(fdt, node, "stdout-path");
}

static inline uint64_t round_up(uint64_t value, uint64_t to) {
  return ((value + to - 1) / to) * to;
}

void fdt_enlarge(void *fdt) {
  int old_size = fdt_totalsize(fdt);
  int new_size = (int)round_up(fdt_totalsize(fdt), PAGE_SIZE) + PAGE_SIZE;
  fdt_open_into(fdt, fdt, new_size);
}

uint64_t fdt_size(void *fdt) { return fdt_totalsize(fdt); }
