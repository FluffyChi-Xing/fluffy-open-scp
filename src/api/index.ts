import axios from 'axios'
import { registerInterceptors } from './interceptors'

export const $request = axios.create({
  baseURL: '/api/v1',
  timeout: 15_000,
})

registerInterceptors($request)
