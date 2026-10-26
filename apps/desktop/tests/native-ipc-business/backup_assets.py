"""Bounded independent oracle for the fixture's public 1x1 normalized PNG assets."""
import binascii
import hashlib
import struct
import zlib

SOURCE_PIXEL = bytes([17, 34, 51, 255])
OLD_PIXEL = bytes([68, 85, 102, 255])


def decode_png(data, expected=None):
    assert isinstance(data, bytes) and 0 < len(data) <= 65536 and data.startswith(b'\x89PNG\r\n\x1a\n'), 'PNG bound/signature'
    offset, names, compressed = 8, [], bytearray()
    while offset < len(data):
        assert offset + 12 <= len(data) and len(names) < 16, 'PNG chunk bound'
        size, = struct.unpack_from('>I', data, offset)
        assert size <= 65536 and offset + 12 + size <= len(data), 'PNG chunk length'
        name, value = data[offset+4:offset+8], data[offset+8:offset+8+size]
        crc, = struct.unpack_from('>I', data, offset+8+size)
        assert crc == binascii.crc32(name + value), 'PNG CRC mismatch'
        assert name in (b'IHDR', b'IDAT', b'IEND'), 'normalization retained metadata or unexpected chunks'
        names.append(name); offset += size + 12
        if name == b'IHDR':
            assert names == [b'IHDR'] and value == struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0), 'unexpected fixture dimensions/color'
        elif name == b'IDAT': compressed.extend(value)
        else:
            assert value == b'' and offset == len(data), 'PNG trailing data'
    assert names[0] == b'IHDR' and names[-1] == b'IEND' and b'IDAT' in names and names.count(b'IEND') == 1
    decoder = zlib.decompressobj()
    try: pixels = decoder.decompress(bytes(compressed), 6)
    except zlib.error as error: raise AssertionError('PNG zlib invalid') from error
    assert decoder.eof and not decoder.unused_data and not decoder.unconsumed_tail and len(pixels) == 5, 'PNG decoded bound'
    # A single RGBA pixel has zero-valued left/up predictors for all five legal filters.
    assert pixels[0] in range(5), 'PNG filter invalid'
    if expected is not None: assert pixels[1:] == expected, 'restored pixel differs from synthetic source'
    return pixels[1:]


def validate_generation(directory, state, expected, filesystem):
    import runner
    assets = state['assets']
    assert isinstance(assets, list) and len(assets) == 1, 'exactly one normalized asset required'
    image = assets[0]
    assert set(image) == {'sha256', 'mime', 'bytes', 'width', 'height'}
    import re
    assert isinstance(image['sha256'], str) and re.fullmatch('[0-9a-f]{64}', image['sha256'])
    assert image['mime'] == 'image/png' and type(image['bytes']) is int and 0 < image['bytes'] <= 65536
    assert type(image['width']) is int and type(image['height']) is int and image['width'] == image['height'] == 1
    assets_dir = directory / 'assets'
    filesystem.append(runner.private(assets_dir, 0o700, True))
    assert set(p.name for p in assets_dir.iterdir()) == {image['sha256']}, 'unexpected active generation assets'
    path = assets_dir / image['sha256']; filesystem.append(runner.private(path, 0o600))
    with path.open('rb') as stream: data = stream.read(65537)
    assert len(data) == image['bytes'] and hashlib.sha256(data).hexdigest() == image['sha256'], 'asset bytes/hash mismatch'
    decode_png(data, expected)
    campaigns = [campaign for slot in state['draft']['slots'] for campaign in slot['campaigns']]
    assert len(campaigns) == 1 and campaigns[0]['image'] == image, 'restored campaign image reference mismatch'
    return image
