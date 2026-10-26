"""Independent disk, preview and bounded codec oracles for synthetic real image IPC."""
import base64
import binascii
import hashlib
import json
import re
import sys

COMMANDS = ['import_ad_image', 'ad_image_preview']
CASES = ['png','jpeg','exact_png','corrupt_png','corrupt_jpeg','unsupported','oversize','missing_preview']
ERRORS = {'corrupt_png':'发布数据无效：image_chunk','corrupt_jpeg':'发布数据无效：image_header',
          'unsupported':'请选择静态 PNG 或 JPEG 图片','oversize':'图片超过 1 MiB 上限','missing_preview':'发布数据无效：asset_missing'}


def validate_public(rows):
    assert rows == [{'command':c,'kind':'permission'} for c in COMMANDS], 'Public image permission denial required'


def validate_error(name, error):
    assert error == ERRORS[name], 'wrong image rejection classification'


def preview_bytes(url, mime):
    assert isinstance(url,str) and len(url) <= 65536 and url.startswith('data:'+mime+';base64,'), 'preview MIME/bound'
    try: data = base64.b64decode(url.split(',',1)[1],validate=True)
    except (ValueError,binascii.Error) as error: raise AssertionError('preview base64 invalid') from error
    assert 0 < len(data) <= 65536
    return data


def jpeg_dimensions(data, normalized=True):
    assert 0 < len(data) <= 65536 and data.startswith(b'\xff\xd8') and data.endswith(b'\xff\xd9'), 'JPEG signature/bound'
    offset, dimensions, segments = 2, None, 0
    while offset < len(data)-2:
        assert data[offset] == 255 and offset+4 <= len(data), 'JPEG marker invalid'
        marker = data[offset+1]; length = int.from_bytes(data[offset+2:offset+4],'big')
        assert length >= 2 and offset+2+length <= len(data) and segments < 128, 'JPEG segment bound'
        value=data[offset+4:offset+2+length]; offset += 2+length; segments += 1
        if normalized: assert marker not in (0xfe,0xe1,0xe2), 'normalization retained COM/EXIF/ICC metadata'
        if marker in (0xc0,0xc1,0xc2):
            assert len(value)>=6 and value[0]==8
            dimensions=(int.from_bytes(value[3:5],'big'),int.from_bytes(value[1:3],'big'))
        if marker==0xda:
            assert dimensions is not None; return dimensions
    raise AssertionError('JPEG scan missing')


def validate_rows(rows, events, initial, png_size, jpeg_size):
    assert [r.get('id') for r in rows]==CASES and events==[r[k] for r in rows for k in ('before','after')], 'image event matrix mismatch'
    sizes=[png_size,jpeg_size,1048576,8,3,6,1048577,0]; previous=None; imported={}
    for index,row in enumerate(rows):
        assert set(row)=={'id','size','accepted','value','preview','error','before','after','draft'}
        assert type(row['size']) is int and row['size']==sizes[index]
        assert row['draft']==initial,'image import changed draft'
        for key in ('before','after'):
            snapshot=row[key];assert set(snapshot)=={'label','files'} and snapshot['label']==row['id']+':'+key
            assert snapshot['files'] and len(snapshot['files'])<=32
            for name,digest in snapshot['files'].items():
                assert not name.startswith('/') and '..' not in name.split('/') and re.fullmatch('[0-9a-f]{64}',digest)
        if previous is not None:assert row['before']['files']==previous,'unobserved image mutation'
        if index<3:
            assert row['accepted'] is True and row['error'] is None
            value=row['value'];assert set(value)=={'asset','preview_data_url'}
            asset=value['asset'];assert set(asset)=={'sha256','mime','bytes','width','height'}
            mime='image/jpeg' if index==1 else 'image/png'
            assert asset['mime']==mime and type(asset['width']) is int and type(asset['height']) is int and asset['width']==asset['height']==1
            assert type(asset['bytes']) is int and 0<asset['bytes']<=65536 and re.fullmatch('[0-9a-f]{64}',asset['sha256'])
            data=preview_bytes(row['preview'],mime)
            assert row['preview']==value['preview_data_url'] and len(data)==asset['bytes'] and hashlib.sha256(data).hexdigest()==asset['sha256']
            if index==2:
                assert value==rows[0]['value'] and row['before']['files']==row['after']['files'],'exact boundary dedupe mismatch'
            imported[asset['sha256']]=(asset,data)
        else:
            assert row['accepted'] is False and row['value'] is None and row['preview'] is None
            validate_error(row['id'],row['error']);assert row['before']['files']==row['after']['files'],'rejected image mutated files'
        previous=row['after']['files']
    assert len(imported)==2
    return imported,previous


def validate(stdout,responses,edition,folder,filesystem):
    import runner
    root=folder/'tmp/business-root'
    if edition=='public':
        assert set(responses)=={'images'};validate_public(responses['images']);assert list(root.iterdir())==[]
        return dict(status='PASS',edition=edition,mode='images',permission_denials=2,no_writes=True)
    assert set(responses)=={'images','initial'}
    prefix='BUSINESS_IPC_DRAFT '
    events=[json.loads(line[len(prefix):]) for line in stdout.splitlines() if line.startswith(prefix)]
    imported,files=validate_rows(responses['images'],events,responses['initial'],(runner.HERE/'backup-source.png').stat().st_size,(runner.HERE/'image-source.jpg').stat().st_size)
    edition_dir=root/'v2/personal';publisher=edition_dir/'publisher'
    assert set((root/'v2').iterdir())=={edition_dir} and set(p.name for p in edition_dir.iterdir())=={'publisher'}
    actual={}
    for path in [root/'v2',edition_dir,publisher]:filesystem.append(runner.private(path,0o700,True))
    for path in publisher.rglob('*'):
        if path.is_dir() and not path.is_symlink():filesystem.append(runner.private(path,0o700,True))
        else:filesystem.append(runner.private(path,0o600));actual[str(path.relative_to(publisher))]=runner.sha(path)
    assert actual==files,'independent final image disk hashes mismatch'
    active=json.loads(runner.bounded_text(publisher/'active.json',4096));assert re.fullmatch('[0-9a-f]{32}',active['generation'])
    generation=publisher/active['generation'];state=json.loads(runner.bounded_text(generation/'state.json',65536))
    assert state['draft']==responses['initial'] and {a['sha256'] for a in state['assets']}==set(imported)
    assert set(p.name for p in (generation/'assets').iterdir())==set(imported)
    checks=[]
    from backup_assets import decode_png,SOURCE_PIXEL
    for asset,data in imported.values():
        path=generation/'assets'/asset['sha256']
        with path.open('rb') as handle:disk=handle.read(65537)
        assert disk==data and asset in state['assets'],'preview differs from normalized disk bytes/metadata'
        if asset['mime']=='image/png':decode_png(disk,SOURCE_PIXEL)
        else:
            assert jpeg_dimensions(disk)==(1,1)
            execution=runner.execute([sys.executable,'-B',str(runner.HERE/'jpeg_oracle.py'),str(path)],folder,20,label='jpeg-oracle')
            assert execution['exit']==0 and not execution['timeout'],'system synthetic JPEG decoder unavailable'
            decoded=json.loads((folder/'jpeg-oracle-stdout.log').read_text())
            assert decoded['decoder']=='ImageIO/CoreGraphics'
            pixel=decoded['pixel'];assert len(pixel)==4 and pixel[3]==255 and all(abs(a-b)<=3 for a,b in zip(pixel,SOURCE_PIXEL[:3])),'JPEG synthetic pixel mismatch'
            checks.append(dict(jpeg_oracle=execution,decoded_pixel_within_tolerance=True))
    return dict(status='PASS',edition=edition,mode='images',accepted=3,rejected=5,normalized_assets=[a for a,d in imported.values()],
                filesystem=filesystem,codec_checks=checks,exact_1MiB_accepted=True,metadata_removed=True,
                intermediate_snapshots='fixture-origin',independent_disk='final bytes/hash/metadata/preview/pixels/permissions',ordinary_app='NOT_RUN',real_keychain='NOT_RUN')
