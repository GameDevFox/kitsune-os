#include <stdint.h>

struct Device_Tree_Header {
  uint32_t magic;
  uint32_t total_size;

  uint32_t structure_offset;
  uint32_t strings_offset;
  uint32_t memory_reservation_offset;

  uint32_t version;
  uint32_t last_compatible_version;

  uint32_t boot_cpu_physical_id;

  uint32_t strings_size;
  uint32_t structure_size;
};
