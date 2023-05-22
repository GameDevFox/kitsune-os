import { readFileSync } from 'fs';

import { ApolloServer } from '@apollo/server'
import { coprocRegisterCodes } from '@kitsune-os/common';

import { Api, images } from './api';
import { spawnSync } from 'child_process';
import { loadSymbols } from './kernel-symbols';

interface Color {
  red: number,
  green: number,
  blue: number,
}

const drawNameEnumValues: string[] = [];
const drawNameResolver: Record<string, string> = {};

Object.keys(images).map((key) => {
  const enumName = key.toUpperCase().replace(/-/g, '_');
  drawNameEnumValues.push(enumName);

  drawNameResolver[enumName] = key;
});

const typeDefs = `
  input Color {
    red: Int!
    green: Int!
    blue: Int!
  }

  type CoprocRegister {
    name: String!
    args: [Int!]!
    isReadable: Boolean!
    isWriteable: Boolean!
  }

  enum DrawName {
    ${drawNameEnumValues.join('\n    ')}
  }

  type Query {
    coprocRegisterNames: [CoprocRegister!]!
    coprocRegister(names: [String!]!): [Int!]!
    memory(start: Int!, length: Int!): [Int!]!
  }

  type Mutation {
    clear: Boolean!
    draw(names: [DrawName!]!): Boolean!
    hello: Boolean!
    printDeviceTree: Boolean!
    printTimer: Boolean!
    setColor(color: Color!): Boolean!

    test: Int!
    flash: Int!
    instructionAbort: Boolean!
    sendBytes(bytes: String!): Boolean!
  }
`;

type None = undefined;

export const build = (api: Api) => {
  const resolvers = {
    DrawName: drawNameResolver,

    Query: {
      coprocRegisterNames: () => {
        return Object.entries(coprocRegisterCodes)
          .map(([name, info]) => ({ name, ...info }));
      },
      coprocRegister: (_parent: None, args: { names: string[] }) => {
        const { names } = args;

        if(names.length) {
          return [];
        }

        return [1, 2, 3, 4];

      },
      memory: (_parent: None, args: { start: number, length: number }) => {
        const { start, length } = args;

        return new Promise((resolve) => {
          api.readMemory(start, length, resolve);
        });
      },
    },

    Mutation: {
      clear: () => {
        api.clear();
        return true;
      },
      draw: (_parent: None, args: { names: string[] }) => {
        const names = args.names;
        names.forEach(name => api.draw(name as any));
        return true;
      },
      hello: () => {
        api.hello();
        return true;
      },
      printDeviceTree: () => {
        api.printDeviceTree();
        return true;
      },
      printTimer: () => {
        api.printTimer();
        return true;
      },
      setColor: (_parent: None, args: { color: Color }) => {
        const { red, green, blue } = args.color;
        api.setColor({ r: red, g: green, b: blue });
        return true;
      },

      test: () => {
        const mascotData = readFileSync('../../image/mascot.data');

        const start = Date.now();
        return new Promise((resolve) => {
          api.writeMemory(0x175054, mascotData, () => {
            const stop = Date.now();
            const delta = stop - start;
            resolve(delta);
          });
        });
      },
      flash: async () => {
        console.log('FLASH');

        const { __data_start } = await loadSymbols();
        console.log('__data_start', __data_start);

        // The whole image is offset at 0x8000
        const skip = __data_start - 0x8000;

        const { stdout: data } = spawnSync(
          'dd', ['if=../../kitsune.img', 'bs=1', `skip=${skip}`]
        );

        console.log('data', data.length, data);

        return new Promise((resolve) => {
          const start = Date.now();
          api.writeMemory(__data_start, data, () => {
            const stop = Date.now();
            const delta = stop - start;
            resolve(delta);
          });
        });
      },
      instructionAbort: () => {
        api.instructionAbort();
        return true;
      },
      sendBytes: (_parent: None, args: { bytes: string }) => {
        const { bytes } = args;
        api.sendBytes(bytes);
        return true;
      }
    },
  };

  const server = new ApolloServer({
    typeDefs,
    resolvers,
  });

  return server;
};
