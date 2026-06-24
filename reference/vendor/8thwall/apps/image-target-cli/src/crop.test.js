import {describe, it} from 'node:test'
import assert from 'node:assert/strict'

import {getDefaultCrop} from './crop.js'

describe('getDefaultCrop', () => {
  it('returns portrait crop for a portrait image', () => {
    const crop = getDefaultCrop({width: 995, height: 1326}, false)
    assert.equal(crop.isRotated, false)
    assert.equal(crop.originalWidth, 995)
    assert.equal(crop.originalHeight, 1326)
    // width/3=331.7, height/4=331.5 → width/3 > height/4 → width cropped
    assert.equal(crop.width, Math.round((1326 * 3) / 4))
    assert.equal(crop.height, 1326)
    assert.ok(crop.left >= 0)
    assert.equal(crop.top, 0)
    assert.ok(crop.left + crop.width <= 995)
  })
  it('returns landscape crop for a portrait image rotated to landscape', () => {
    const crop = getDefaultCrop({width: 995, height: 1326}, true)
    assert.equal(crop.isRotated, true)
    // originalWidth/Height are post-rotation dimensions
    assert.equal(crop.originalWidth, 1326)
    assert.equal(crop.originalHeight, 995)
    // After rotation: effective dimensions are 1326x995
    // 3:4 aspect ratio for landscape: height stays, width cropped
    assert.equal(crop.height, 995)
    assert.equal(crop.width, Math.round((995 * 3) / 4))
    assert.ok(crop.left >= 0)
    assert.equal(crop.top, 0)
    // Crop must fit within rotated dimensions (1326x995)
    assert.ok(crop.left + crop.width <= 1326)
  })
  it('returns landscape crop for a landscape image', () => {
    const crop = getDefaultCrop({width: 1600, height: 900}, true)
    assert.equal(crop.isRotated, true)
    // originalWidth/Height are post-rotation: 1600x900 rotated -> 900x1600
    assert.equal(crop.originalWidth, 900)
    assert.equal(crop.originalHeight, 1600)
    // After rotation: effective dimensions are 900x1600
    assert.ok(crop.left + crop.width <= 900)
    assert.ok(crop.top + crop.height <= 1600)
  })
  it('returns portrait crop for a square image', () => {
    const crop = getDefaultCrop({width: 1000, height: 1000}, false)
    assert.equal(crop.originalWidth, 1000)
    assert.equal(crop.originalHeight, 1000)
    // width/3=333, height/4=250, width/3 > height/4 → width cropped
    assert.equal(crop.width, Math.round((1000 * 3) / 4))
    assert.equal(crop.height, 1000)
  })
})
