// REGISTER_NAME: [coproc, opcode1, CRn, CRm, opcode2]

import { CoprocRegister, Field } from "./types";

import { sctlr } from "./coprocs/sctlr";
import { id_pfr0 } from "./coprocs/id_pfr0";

// CR = coprocessor register

export const getFieldForBit = (fields: Field[], bitIndex: number) => {
  return fields.find(field => {
    const { startBit, length } = field;
    const endBit = startBit + length;

    return bitIndex >= startBit && bitIndex < endBit;
  });
};

export const coprocRegisterCodes: Record<string, CoprocRegister> = {
  "ACTLR": {
    args: [15,0,1,0,1],
    isReadable: true,
    isWriteable: true,
  },
  "ADFSR": {
    args: [15,0,5,1,0],
    isReadable: true,
    isWriteable: true,
  },
  "AIFSR": {
    args: [15,0,5,1,1],
    isReadable: true,
    isWriteable: true,
  },
  "AIDR": {
    args: [15,1,0,0,7],
    isReadable: true,
    isWriteable: false,
  },
  "AMAIR0": {
    args: [15,0,10,3,0],
    isReadable: true,
    isWriteable: false,
  },
  "AMAIR1": {
    args: [15,0,10,3,1],
    isReadable: false,
    isWriteable: true,
  },
  "CCSIDR": {
    args: [15,1,0,0,0],
    isReadable: true,
    isWriteable: false,
  },
  "CLIDR": {
    args: [15,1,0,0,1],
    isReadable: true,
    isWriteable: false,
  },
  "CNTFRQ": {
    args: [15,0,14,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "CNTHCTL": {
    args: [15,4,14,1,0],
    isReadable: true,
    isWriteable: true,
  },
  "CNTHP_CTL": {
    args: [15,4,14,2,1],
    isReadable: true,
    isWriteable: true,
  },
  "CNTHP_TVAL": {
    args: [15,4,14,2,0],
    isReadable: true,
    isWriteable: true,
  },
  "CNTKCTL": {
    args: [15,0,14,1,0],
    isReadable: true,
    isWriteable: false,
  },
  "CNTKCT": {
    args: [15,0,14,1,0],
    isReadable: false,
    isWriteable: true,
  },
  "CNTP_CTL": {
    args: [15,0,14,2,1],
    isReadable: true,
    isWriteable: true,
  },
  "CNTP_TVAL": {
    args: [15,0,14,2,0],
    isReadable: true,
    isWriteable: true,
  },
  "CNTV_CTL": {
    args: [15,0,14,3,1],
    isReadable: true,
    isWriteable: true,
  },
  "CNTV_TVAL": {
    args: [15,0,14,3,0],
    isReadable: true,
    isWriteable: true,
  },
  "CONTEXTIDR": {
    args: [15,0,13,0,1],
    isReadable: true,
    isWriteable: true,
  },
  "CPACR": {
    args: [15,0,1,0,2],
    isReadable: true,
    isWriteable: true,
  },
  "CSSELR": {
    args: [15,2,0,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "CTR": {
    args: [15,0,0,0,1],
    isReadable: true,
    isWriteable: false,
  },
  "DACR": {
    args: [15,0,3,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "DFAR": {
    args: [15,0,6,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "DFSR": {
    args: [15,0,5,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "FCSEIDR": {
    args: [15,0,13,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "HACR": {
    args: [15,4,1,1,7],
    isReadable: true,
    isWriteable: true,
  },
  "HACTLR": {
    args: [15,4,1,0,1],
    isReadable: true,
    isWriteable: true,
  },
  "HADFSR": {
    args: [15,4,5,1,0],
    isReadable: true,
    isWriteable: true,
  },
  "HAIFSR": {
    args: [15,4,5,1,1],
    isReadable: true,
    isWriteable: true,
  },
  "HAMAIR0": {
    args: [15,4,10,3,0],
    isReadable: true,
    isWriteable: false,
  },
  "HAMAIR1": {
    args: [15,4,10,3,1],
    isReadable: false,
    isWriteable: true,
  },
  "HCPTR": {
    args: [15,4,1,1,2],
    isReadable: true,
    isWriteable: true,
  },
  "HCR": {
    args: [15,4,1,1,0],
    isReadable: true,
    isWriteable: true,
  },
  "HDCR": {
    args: [15,4,1,1,1],
    isReadable: true,
    isWriteable: true,
  },
  "HDFAR": {
    args: [15,4,6,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "HIFAR": {
    args: [15,4,6,0,2],
    isReadable: true,
    isWriteable: true,
  },
  "HMAIR0": {
    args: [15,4,10,2,0],
    isReadable: true,
    isWriteable: false,
  },
  "HMAIR1": {
    args: [15,4,10,2,1],
    isReadable: false,
    isWriteable: true,
  },
  "HPFAR": {
    args: [15,4,6,0,4],
    isReadable: true,
    isWriteable: true,
  },
  "HSCTLR": {
    args: [15,4,1,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "HSR": {
    args: [15,4,5,2,0],
    isReadable: true,
    isWriteable: true,
  },
  "HSTR": {
    args: [15,4,1,1,3],
    isReadable: true,
    isWriteable: true,
  },
  "HTCR": {
    args: [15,4,2,0,2],
    isReadable: true,
    isWriteable: true,
  },
  "HTPIDR": {
    args: [15,4,13,0,2],
    isReadable: true,
    isWriteable: true,
  },
  "HVBAR": {
    args: [15,4,12,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "ID_AFR0": {
    args: [15,0,0,1,3],
    isReadable: true,
    isWriteable: false,
  },
  "ID_DFR0": {
    args: [15,0,0,1,2],
    isReadable: true,
    isWriteable: false,
  },
  "ID_ISAR0": {
    args: [15,0,0,2,0],
    isReadable: true,
    isWriteable: false,
  },
  "ID_ISAR1": {
    args: [15,0,0,2,1],
    isReadable: true,
    isWriteable: false,
  },
  "ID_ISAR2": {
    args: [15,0,0,2,2],
    isReadable: true,
    isWriteable: false,
  },
  "ID_ISAR3": {
    args: [15,0,0,2,3],
    isReadable: true,
    isWriteable: false,
  },
  "ID_ISAR4": {
    args: [15,0,0,2,4],
    isReadable: true,
    isWriteable: false,
  },
  "ID_ISAR5": {
    args: [15,0,0,2,5],
    isReadable: true,
    isWriteable: false,
  },
  "ID_MMFR0": {
    args: [15,0,0,1,4],
    isReadable: true,
    isWriteable: false,
  },
  "ID_MMFR1": {
    args: [15,0,0,1,5],
    isReadable: true,
    isWriteable: false,
  },
  "ID_MMFR2": {
    args: [15,0,0,1,6],
    isReadable: true,
    isWriteable: false,
  },
  "ID_MMFR3": {
    args: [15,0,0,1,7],
    isReadable: true,
    isWriteable: false,
  },
  "ID_PFR0": id_pfr0,
  "ID_PFR1": {
    args: [15,0,0,1,1],
    isReadable: true,
    isWriteable: false,
  },
  "IFAR": {
    args: [15,0,6,0,2],
    isReadable: true,
    isWriteable: true,
  },
  "IFSR": {
    args: [15,0,5,0,1],
    isReadable: true,
    isWriteable: true,
  },
  "JIDR": {
    args: [14,7,0,0,0],
    isReadable: true,
    isWriteable: false,
  },
  "JMCR": {
    args: [14,7,2,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "JOSCR": {
    args: [14,7,1,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "MAIR0": {
    args: [15,0,10,2,0],
    isReadable: true,
    isWriteable: false,
  },
  "MAIR1": {
    args: [15,0,10,2,1],
    isReadable: false,
    isWriteable: true,
  },
  "MIDR": {
    args: [15,0,0,0,0],
    isReadable: true,
    isWriteable: false,
  },
  "MPIDR": {
    args: [15,0,0,0,5],
    isReadable: true,
    isWriteable: false,
  },
  "MVBAR": {
    args: [15,0,12,0,1],
    isReadable: true,
    isWriteable: true,
  },
  "NMRR": {
    args: [15,0,10,2,1],
    isReadable: true,
    isWriteable: true,
  },
  "NSACR": {
    args: [15,0,1,1,2],
    isReadable: true,
    isWriteable: true,
  },
  "PAR[31:0]": {
    args: [15,0,7,4,0],
    isReadable: true,
    isWriteable: true,
  },
  "PMCCNTR": {
    args: [15,0,9,13,0],
    isReadable: true,
    isWriteable: true,
  },
  "PMCEID0": {
    args: [15,0,9,12,6],
    isReadable: true,
    isWriteable: false,
  },
  "PMCNTENCLR": {
    args: [15,0,9,12,2],
    isReadable: true,
    isWriteable: true,
  },
  "PMCNTENSET": {
    args: [15,0,9,12,1],
    isReadable: true,
    isWriteable: true,
  },
  "PMCR": {
    args: [15,0,9,12,0],
    isReadable: true,
    isWriteable: true,
  },
  "PMINTENCLR": {
    args: [15,0,9,14,2],
    isReadable: true,
    isWriteable: true,
  },
  "PMINTENSET": {
    args: [15,0,9,14,1],
    isReadable: true,
    isWriteable: true,
  },
  "PMOVSR": {
    args: [15,0,9,12,3],
    isReadable: true,
    isWriteable: true,
  },
  "PMOVSSET": {
    args: [15,0,9,14,3],
    isReadable: true,
    isWriteable: true,
  },
  "PMSELR": {
    args: [15,0,9,12,5],
    isReadable: true,
    isWriteable: true,
  },
  "PMSWINC": {
    args: [15,0,9,12,4],
    isReadable: false,
    isWriteable: true,
  },
  "PMUSERENR": {
    args: [15,0,9,14,0],
    isReadable: true,
    isWriteable: true,
  },
  "PMXEVCNTR": {
    args: [15,0,9,13,2],
    isReadable: true,
    isWriteable: true,
  },
  "PMXEVTYPER": {
    args: [15,0,9,13,1],
    isReadable: true,
    isWriteable: true,
  },
  "PRRR": {
    args: [15,0,10,2,0],
    isReadable: true,
    isWriteable: true,
  },
  "REVIDR": {
    args: [15,0,0,0,6],
    isReadable: true,
    isWriteable: false,
  },
  "SCR": {
    args: [15,0,1,1,0],
    isReadable: true,
    isWriteable: true,
  },
  "SCTLR": sctlr,
  "SDER": {
    args: [15,0,1,1,1],
    isReadable: true,
    isWriteable: true,
  },
  "TCMTR": {
    args: [15,0,0,0,2],
    isReadable: true,
    isWriteable: false,
  },
  "TEECR": {
    args: [14,6,0,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "TEEHBR": {
    args: [14,6,1,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "TLBTR": {
    args: [15,0,0,0,3],
    isReadable: true,
    isWriteable: false,
  },
  "TPIDRPRW": {
    args: [15,0,13,0,4],
    isReadable: true,
    isWriteable: true,
  },
  "TPIDRURO": {
    args: [15,0,13,0,3],
    isReadable: true,
    isWriteable: true,
  },
  "TPIDRURW": {
    args: [15,0,13,0,2],
    isReadable: true,
    isWriteable: true,
  },
  "TTBCR": {
    args: [15,0,2,0,2],
    isReadable: true,
    isWriteable: true,
  },
  "TTBR0": {
    args: [15,0,2,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "TTBR1": {
    args: [15,0,2,0,1],
    isReadable: true,
    isWriteable: true,
  },
  "VBAR": {
    args: [15,0,12,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "VMPIDR": {
    args: [15,4,0,0,5],
    isReadable: true,
    isWriteable: true,
  },
  "VPIDR": {
    args: [15,4,0,0,0],
    isReadable: true,
    isWriteable: true,
  },
  "VTCR": {
    args: [15,4,2,1,2],
    isReadable: true,
    isWriteable: true,
  },
}
