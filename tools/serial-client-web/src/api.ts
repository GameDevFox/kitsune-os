import axios from 'axios';

const baseURL = 'http://localhost:8080';

export const api = axios.create({ baseURL });

export const sayHello = () => api.get('/hello');
export const clear = () => api.get('/clear');
export const draw = (value: string) => api.get(`/draw/${value}`);
export const printDeviceTree = () => api.get('/print-device-tree');

export const listCoprocRegs = () => api.get('/coproc-registers');
export const readCoprocReg = (name: string) => api.get(`/coproc-registers/${name}`);
export const writeCoprocReg = (name: string, value: string) => api.post(`/coproc-registers/${name}`, { value });

export const readCPSR = () => api.get('/cpsr');
export const writeCPSR = (value: number[]) => api.post('/cpsr', { value });
