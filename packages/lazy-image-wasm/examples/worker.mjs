import { createUploadWorkerHandler } from '../worker.js';

self.addEventListener('message', createUploadWorkerHandler());
