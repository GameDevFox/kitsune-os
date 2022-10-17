import axios from 'axios';

const baseURL = 'http://localhost:8080';

export const api = axios.create({ baseURL });

export const sayHello = () => api.get('/hello');
export const clear = () => api.get('/clear');
export const draw = (value: string) => api.get(`/draw/${value}`);
export const listCoprocRegs = () => api.get(`/coproc-registers`);
export const readCoprocReg = (name: string) => api.get(`/coproc-registers/${name}`);
