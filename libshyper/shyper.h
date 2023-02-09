#ifndef SHYPER_H
#define SHYPER_H

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

const size_t NAME_MAX_LEN = 32;

// for VM info
struct VMInfo {
    uint32_t id;
    uint8_t vm_name[NAME_MAX_LEN];
    uint32_t vm_type;
    uint32_t vm_state;
};

// for mediated block
struct MediatedBlkCfg {
    uint8_t name[NAME_MAX_LEN];
    uint8_t block_dev_path[NAME_MAX_LEN];
    size_t block_num;
    size_t dma_block_max;
    size_t cache_size;
    uint16_t idx;
    bool pcache;
    size_t cache_va;
    size_t cache_ipa;
    size_t cache_pa;
};

struct MediatedBlkReq {
    uint32_t req_type;
    size_t sector;
    size_t count;
};

struct MediatedBlkContent {
    size_t nreq;
    struct MediatedBlkCfg cfg;
    struct MediatedBlkReq req;
};

#ifdef __cplusplus
}
#endif

#endif
