import { Field, fieldUtils } from "@kitsune-os/common";

const { a, f } = fieldUtils;

const exceptionMaskValues = [
  { value: 0, description: 'Exception not masked' },
  { value: 1, description: 'Exception masked' },
];

export const fields: Field[] = [
  f(
    0, 4, 'M', 'Mode', '',
    [
      { value: 0x10, description: 'User (USR)' },
      { value: 0x11, description: 'FIQ' },
      { value: 0x12, description: 'IRQ' },
      { value: 0x13, description: 'Supervisor (SVC)' },

      { value: 0x16, description: 'Monitor (MON)' },
      { value: 0x17, description: 'Abort (ABT)' },

      { value: 0x1a, description: 'Hyp (HYP)' },

      { value: 0x1b, description: 'Undef (UND)' },
      { value: 0x1f, description: 'System (SYS)' },
    ],
  ),
  f(5, 5, 'T', 'Thumb execution state bit', ''),
  f(6, 6, 'F', 'FIQ mask bit', '', exceptionMaskValues),
  f(7, 7, 'I', 'IRQ mask bit', '', exceptionMaskValues),
  f(8, 8, 'A', 'Asynchronous abort mask bit', '', exceptionMaskValues),
  f(
    9, 9, 'E', 'Endianness execution state bit',
    'Controls the load and store endianness for data accesses',
    [
      { value: 0, description: 'Little-endian operation' },
      { value: 1, description: 'Big-endian operation' },
    ]
  ),
  f(10, 15, 'IT', 'bits[7:2] If-Then execution state bits for the Thumb IT (If-Then) instruction', ''),
  f(
    16, 19, 'GE', 'Greater than or Equal flags',
    'For the parallel addition and subtraction (SIMD) instructions described ' +
      'in Parallel addition and subtraction instructions on page A4-169',
  ),
  a(20, 23, 'RAZ'),
  f(24, 24, 'J', 'Jazelle bit', 'See the description of the T bit, bit[5]'),
  f(25, 26, 'IT', 'bits[1:0] If-Then execution state bits for the Thumb IT (If-Then) instruction', ''),
  f(
    27, 27, 'Q', 'Cumulative saturation bit',
    'This bit can be read or written in any mode, and is described in The ' +
      'Application Program Status Register (APSR) on page A2-49'
  ),
  f(28, 28, 'V', 'Overflow condition flag', ''),
  f(29, 29, 'C', 'Carry condition flag', ''),
  f(30, 30, 'Z', 'Zero condition flag', ''),
  f(31, 31, 'N', 'Negative condition flag', ''),
];
