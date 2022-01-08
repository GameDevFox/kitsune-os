#define ASM_MRC_FLAG (1 << 20)

uint32_t asm_b(int32_t offset);
uint32_t asm_bx(char reg);

size_t asm_mrc_mcr(
	char coproc, char opc1, char Rt,
	char CRn, char CRm, char opc2, uint32_t flags
);

#define asm_mrc(coproc, opc1, Rt, CRn, CRm, opc2) \
	asm_mrc_mcr(coproc, opc1, Rt, CRn, CRm, opc2, ASM_MRC_FLAG)

#define asm_mcr(coproc, opc1, Rt, CRn, CRm, opc2) \
	asm_mrc_mcr(coproc, opc1, Rt, CRn, CRm, opc2, 0)

#define SWI(value) "swi #" #value
