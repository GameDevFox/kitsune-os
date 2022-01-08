struct deviceTreeHeader {
	uint32_t magic;
	uint32_t totalSize;

	uint32_t structureOffset;
	uint32_t stringsOffset;
	uint32_t memoryReservationOffset;

	uint32_t version;
	uint32_t lastCompatibleVersion;

	uint32_t bootCpuPhysicalId;

	uint32_t stringsSize;
	uint32_t structureSize;
};
