"""Decode one bounded synthetic pixel using native ImageIO, without temp files."""
import ctypes as C
import json
import sys
from pathlib import Path


def function(library, name, result, *arguments):
    value=getattr(library,name)
    value.restype=result
    value.argtypes=list(arguments)
    return value


def decode(data):
    if not 0<len(data)<=65536:raise ValueError('synthetic image byte bound')
    cf=C.CDLL('/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation')
    io=C.CDLL('/System/Library/Frameworks/ImageIO.framework/ImageIO')
    cg=C.CDLL('/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics')
    pointer=C.c_void_p;size=C.c_size_t
    release=function(cf,'CFRelease',None,pointer)
    source_release=release
    cfdata=function(cf,'CFDataCreate',pointer,pointer,pointer,C.c_long)(None,data,len(data))
    source=image=space=context=None
    try:
        source=function(io,'CGImageSourceCreateWithData',pointer,pointer,pointer)(cfdata,None)
        if not source:raise ValueError('image source invalid')
        image=function(io,'CGImageSourceCreateImageAtIndex',pointer,pointer,size,pointer)(source,0,None)
        if not image:raise ValueError('image decode invalid')
        width=function(cg,'CGImageGetWidth',size,pointer)(image)
        height=function(cg,'CGImageGetHeight',size,pointer)(image)
        if (width,height)!=(1,1):raise ValueError('synthetic dimensions required')
        space=function(cg,'CGColorSpaceCreateDeviceRGB',pointer)()
        pixel=(C.c_ubyte*4)()
        context=function(cg,'CGBitmapContextCreate',pointer,pointer,size,size,size,size,pointer,C.c_uint)(pixel,1,1,8,4,space,0x4001)
        if not context:raise ValueError('pixel context invalid')
        class Point(C.Structure):_fields_=[('x',C.c_double),('y',C.c_double)]
        class Size(C.Structure):_fields_=[('width',C.c_double),('height',C.c_double)]
        class Rect(C.Structure):_fields_=[('origin',Point),('size',Size)]
        function(cg,'CGContextDrawImage',None,pointer,Rect,pointer)(context,Rect(Point(0,0),Size(1,1)),image)
        return list(pixel)
    finally:
        if context:function(cg,'CGContextRelease',None,pointer)(context)
        if space:function(cg,'CGColorSpaceRelease',None,pointer)(space)
        if image:function(cg,'CGImageRelease',None,pointer)(image)
        if source:source_release(source)
        if cfdata:release(cfdata)


if __name__=='__main__':
    with Path(sys.argv[1]).open('rb') as handle:data=handle.read(65537)
    print(json.dumps({'pixel':decode(data),'decoder':'ImageIO/CoreGraphics'}))
