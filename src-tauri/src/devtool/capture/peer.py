"""Identify the client end of one existing IPv4 loopback connection on Windows."""
import ctypes as c
from ctypes import wintypes as w
import socket,struct
from pathlib import Path

def client_identity(client_port, server_port):
    table = c.windll.iphlpapi.GetExtendedTcpTable
    size = w.DWORD()
    if table(None,c.byref(size),False,2,5,0)!=122:return None
    for _ in range(3):
        buf=c.create_string_buffer(size.value)
        result=table(buf,c.byref(size),False,2,5,0)
        if result!=122:break
    if result:return None
    loop=int.from_bytes(socket.inet_aton("127.0.0.1"),"little")
    for index in range(struct.unpack_from("<I",buf)[0]):
        _,local,lp,remote,rp,pid=struct.unpack_from("<6I",buf,4+index*24)
        if (local,socket.ntohs(lp & 65535),remote,socket.ntohs(rp & 65535))!=(loop,client_port,loop,server_port):continue
        kernel=c.WinDLL("kernel32",use_last_error=True)
        kernel.OpenProcess.argtypes=[w.DWORD,w.BOOL,w.DWORD];kernel.OpenProcess.restype=w.HANDLE
        kernel.QueryFullProcessImageNameW.argtypes=[w.HANDLE,w.DWORD,w.LPWSTR,c.POINTER(w.DWORD)]
        kernel.CloseHandle.argtypes=[w.HANDLE]
        handle=kernel.OpenProcess(0x1000,False,pid)
        if not handle:return None
        try:
            count=w.DWORD(32768);path=c.create_unicode_buffer(count.value)
            if kernel.QueryFullProcessImageNameW(handle,0,path,c.byref(count)):
                return {"pid":pid,"executable":str(Path(path.value))}
        finally:kernel.CloseHandle(handle)
    return None
