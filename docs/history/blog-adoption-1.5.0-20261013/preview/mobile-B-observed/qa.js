addEventListener('load',async()=>{
 const imgs=[...document.images];
 let decodeError=null;try{await Promise.all(imgs.map(image=>image.decode()));}catch(error){decodeError=String(error);}
 const version=navigator.userAgentData?await navigator.userAgentData.getHighEntropyValues(['fullVersionList']):null;
 const observation={url:location.href,viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio},userAgent:navigator.userAgent,browserVersion:version,decodeError,images:imgs.map(image=>({id:image.dataset.imageId||image.alt,currentSrc:image.currentSrc,src:image.getAttribute('src'),srcset:image.getAttribute('srcset'),sizes:image.sizes,complete:image.complete,naturalWidth:image.naturalWidth,naturalHeight:image.naturalHeight,rect:{width:image.getBoundingClientRect().width,height:image.getBoundingClientRect().height},placeholder:getComputedStyle(image).backgroundImage})),resources:performance.getEntriesByType('resource').map(resource=>({name:resource.name,type:resource.initiatorType,transferSize:resource.transferSize,encodedBodySize:resource.encodedBodySize,decodedBodySize:resource.decodedBodySize,responseStatus:resource.responseStatus}))};
 document.querySelector('#qa-observation').textContent=JSON.stringify(observation,null,2);
});
