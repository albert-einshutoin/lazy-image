/**
 * Copyright 2020 Google Inc. All Rights Reserved.
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *     http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
import { defaultOptions } from './meta.js';
import { initEmscriptenModule } from './utils.js';
import { simd } from 'wasm-feature-detect';
let emscriptenModule;
export async function init(module, moduleOptionOverrides) {
    let actualModule = module;
    let actualOptions = moduleOptionOverrides;
    // If only one argument is provided and it's not a WebAssembly.Module
    if (arguments.length === 1 && !(module instanceof WebAssembly.Module)) {
        actualModule = undefined;
        actualOptions = module;
    }
    if (await simd()) {
        const webpEncoder = await import('./codec/enc/webp_enc_simd.js');
        emscriptenModule = initEmscriptenModule(webpEncoder.default, actualModule, actualOptions);
        return emscriptenModule;
    }
    const webpEncoder = await import('./codec/enc/webp_enc.js');
    emscriptenModule = initEmscriptenModule(webpEncoder.default, actualModule, actualOptions);
    return emscriptenModule;
}
export default async function encode(data, options = {}) {
    if (!emscriptenModule)
        emscriptenModule = init();
    const _options = { ...defaultOptions, ...options };
    const module = await emscriptenModule;
    const result = module.encode(data.data, data.width, data.height, _options);
    if (!result)
        throw new Error('Encoding error.');
    return result.buffer;
}
