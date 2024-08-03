import { spawn } from 'child_process';
import { exitCode } from 'process';

export const loadSymbols = () => new Promise<Record<string, number>>((resolve, reject) => {
  const kernelPath = `${__dirname}/../../kernel/kitsune-qemu.elf`;

  const proc = spawn('nm', [kernelPath]);

  let stdout = '';
  proc.stdout.on('data', data => {
    stdout += data.toString();
  });

  proc.on('exit', () => {
    if(proc.exitCode !== 0) {
      reject(`Failed to load symbols. Exit Code: ${exitCode}.`);
      return;
    }

    const lines = stdout.split(/\n/);

    const result: Record<string, number>  = {};
    lines
      .map(line => line.split(/\s/) )
      .filter(([_address, _type, name]) => name && !name.startsWith('$'))
      .filter(([_address, type, _name]) => type !== 'A')
      .forEach(([address, _type, name]) => {
        result[name] = parseInt(`0x${address}`);
      });

    resolve(result);
  });
});
