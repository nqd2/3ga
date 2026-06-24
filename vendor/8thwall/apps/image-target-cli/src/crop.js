import {createRequire} from 'module'

const require = createRequire(import.meta.url)
const CONSTANTS = require('./constants.json')

/**
 * @param {import("./types").ImageMetadata} metadata
 * @param {boolean} isRotated
 * @returns {import("./types").CropGeometry}
 */
const getDefaultCrop = (metadata, isRotated) => {
  const [width, height] = isRotated
    ? [metadata.height, metadata.width]
    : [metadata.width, metadata.height]

  if (width / 3 > height / 4) {
    const croppedWidth = Math.round((height * 3) / 4)
    return {
      left: Math.round((width - croppedWidth) / 2),
      top: 0,
      width: croppedWidth,
      height,
      isRotated,
      originalWidth: width,
      originalHeight: height,
    }
  } else {
    const croppedHeight = Math.round((width * 4) / 3)
    return {
      left: 0,
      top: Math.round((height - croppedHeight) / 2),
      width,
      height: croppedHeight,
      isRotated,
      originalWidth: width,
      originalHeight: height,
    }
  }
}

/**
 * @param {import("./types").CropGeometry} crop
 * @param {import("./types").ImageMetadata} imageMetadata
 * @returns {string[]}
 */
const validateCrop = (crop, imageMetadata) => {
  const issues = []
  const {top, left, width, height} = crop
  if (top < 0) {
    issues.push('Top offset cannot be negative')
  }
  if (left < 0) {
    issues.push('Left offset cannot be negative')
  }
  if (width < CONSTANTS.minimumWidth) {
    issues.push(`Width must be at least ${CONSTANTS.minimumWidth}`)
  }
  if (height < CONSTANTS.minimumHeight) {
    issues.push(`Height must be at least ${CONSTANTS.minimumHeight}`)
  }
  if (top + height > imageMetadata.height) {
    issues.push(
      `Bottom edge of crop exceeds image height (${top + height} > ${imageMetadata.height})`
    )
  }
  if (left + width > imageMetadata.width) {
    issues.push(
      `Right edge of crop exceeds image width (${left + width} > ${imageMetadata.width})`
    )
  }
  return issues
}

export {getDefaultCrop, validateCrop}
