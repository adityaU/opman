function extractImagesFromAnthropicFormat(content) {
    const images = [];
    for (const item of content) {
        if (item.type === 'image' && item.source?.type === 'base64') {
            images.push({
                mediaType: item.source.media_type || 'image/jpeg',
                data: item.source.data
            });
        }
    }
    return images;
}
function extractImagesFromOpenAI(content) {
    const images = [];
    for (const item of content) {
        if (item.type === 'image_url' && item.image_url?.url) {
            const url = item.image_url.url;
            if (url.startsWith('data:')) {
                try {
                    const [header, data] = url.split(',', 2);
                    if (!data)
                        continue;
                    const mediaType = header.split(';')[0].replace('data:', '');
                    images.push({
                        mediaType: mediaType || 'image/jpeg',
                        data: data
                    });
                }
                catch (e) {
                    continue;
                }
            }
        }
    }
    return images;
}
export function extractAllImages(content) {
    if (!Array.isArray(content))
        return [];
    return [...extractImagesFromAnthropicFormat(content), ...extractImagesFromOpenAI(content)];
}
export function convertImagesToKiroFormat(images) {
    return images.map((img) => {
        const format = img.mediaType.split('/')[1] || 'png';
        const binaryString = atob(img.data);
        const bytes = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }
        return {
            format,
            source: {
                bytes
            }
        };
    });
}
export function extractTextFromParts(parts) {
    const textParts = [];
    for (const part of parts) {
        if (part.text && typeof part.text === 'string') {
            textParts.push(part.text);
        }
        else if (part.type === 'text' && part.text) {
            textParts.push(part.text);
        }
    }
    return textParts.join('');
}
