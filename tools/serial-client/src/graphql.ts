import { ApolloServer } from '@apollo/server'

import { coprocRegisterCodes } from '@kitsune-os/common';

import { Api, images } from './api';

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
    },
  };

  const server = new ApolloServer({
    typeDefs,
    resolvers,
  });

  return server;
};
