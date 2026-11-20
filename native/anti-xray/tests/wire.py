"""Vanilla 26.3 wire client and focused terrain packet decoder for the acceptance runs."""

from dataclasses import dataclass
import socket
import struct
import threading
import time
import zlib

import lab
from vanilla_login import Connection, string
from wire_stream import Reader

BLOCK_ENTITY = 6
BLOCK_UPDATE = 8
CHUNK = 46
SECTION_UPDATE = 86
ACK = 4


@dataclass(frozen=True)
class Packet:
    at: float
    kind: int
    data: bytes
    wire_bytes: int


class Client(Connection):
    def __init__(self, port, name, on_chunk=None, *, view_distance=16):
        super().__init__('127.0.0.1', port)
        self.name = name
        self.on_chunk = on_chunk
        self.packets = []
        self.changed = threading.Condition()
        self.failure = None
        self.position = (0.0, 0.0, 0.0)
        self.rotation = (0.0, 0.0)
        self.closing = False
        self.write_lock = threading.Lock()
        self.send(0, lab.varint(777)
                  + string('127.0.0.1') + struct.pack('>H', port) + b'\x02')
        self.send(0, string(name) + lab.offline_uuid(name).bytes)
        while True:
            packet = self.packet()
            if packet.kind == 3:
                self.threshold = Reader(packet.data).varint()
            elif packet.kind == 2:
                break
            elif packet.kind == 0:
                raise RuntimeError(f'{name}: login rejected: {packet.data!r}')
        self.send(3)
        self.send(0, string('en_us') + bytes([view_distance, 0, 1, 127, 1, 0, 1, 0]))
        while True:
            packet = self.packet()
            if packet.kind == 15:
                self.send(7, b'\0')
            elif packet.kind == 4 or packet.kind == 5:
                self.send(packet.kind, packet.data)
            elif packet.kind == 3:
                self.send(3)
                break
            elif packet.kind == 2:
                raise RuntimeError(f'{name}: configuration rejected: {packet.data!r}')
        self.sock.settimeout(None)
        self.thread = threading.Thread(target=self.read_play, daemon=True)
        self.thread.start()

    def send(self, packet_id, payload=b''):
        with self.write_lock:
            super().send(packet_id, payload)

    def packet(self):
        length = self.read_varint()
        frame = self.read_exact(length)
        if self.threshold >= 0:
            pos, size = lab.parse_varint(frame, 0)
            frame = zlib.decompress(frame[pos:]) if size else frame[pos:]
        pos, kind = lab.parse_varint(frame, 0)
        return Packet(time.monotonic(), kind, frame[pos:], length + len(lab.varint(length)))

    def read_play(self):
        try:
            while True:
                packet = self.packet()
                if packet.kind == 45:  # keep-alive
                    self.send(28, packet.data)
                elif packet.kind == 73:  # position
                    reader = Reader(packet.data)
                    teleport = reader.varint()
                    x, y, z, yaw, pitch, flags = struct.unpack('>ddd24xffI', reader.take(60))
                    previous = (*self.position, *self.rotation)
                    values = [value + previous[i] if flags & (1 << i) else value
                              for i, value in enumerate((x, y, z, yaw, pitch))]
                    self.position = tuple(values[:3])
                    self.rotation = tuple(values[3:])
                    self.send(0, lab.varint(teleport) + struct.pack('>dddff', *values))
                    self.send(44)  # PlayerLoaded
                elif packet.kind == 11:  # batch finished
                    self.send(11, struct.pack('>f', 25.0))
                elif packet.kind == 32:
                    raise RuntimeError(f'{self.name}: disconnected: {packet.data!r}')
                if packet.kind == CHUNK and self.on_chunk is not None:
                    self.on_chunk(packet)
                elif self.on_chunk is None or packet.kind in (ACK, BLOCK_UPDATE, SECTION_UPDATE, BLOCK_ENTITY, 73):
                    with self.changed:
                        self.packets.append(packet)
                        self.changed.notify_all()
        except (EOFError, OSError, RuntimeError) as error:
            with self.changed:
                if not self.closing:
                    self.failure = error
                self.changed.notify_all()

    def mark(self):
        with self.changed:
            return len(self.packets)

    def wait(self, predicate, since=0, timeout=35):
        end = time.monotonic() + timeout
        with self.changed:
            while True:
                for index in range(since, len(self.packets)):
                    packet = self.packets[index]
                    if predicate(packet):
                        return index, packet
                since = len(self.packets)
                if self.failure is not None:
                    raise self.failure
                remaining = end - time.monotonic()
                if remaining <= 0:
                    recent = [(p.kind, p.data[:30].hex()) for p in self.packets[-50:]
                              if p.kind in (ACK, BLOCK_UPDATE, BLOCK_ENTITY, SECTION_UPDATE, 73)]
                    raise TimeoutError(f'{self.name}: expected packet did not arrive; recent updates: {recent}')
                self.changed.wait(remaining)

    def close(self):
        self.closing = True
        try:
            self.sock.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.thread.join(5)
        self.sock.close()

    def dig(self, pos, sequence, action=0, face=2):
        x, y, z = pos
        packed = ((x & 0x3ffffff) << 38) | ((z & 0x3ffffff) << 12) | (y & 0xfff)
        self.send(41, lab.varint(action) + struct.pack('>Q', packed) + bytes([face])
                  + lab.varint(sequence))

    def move(self, x, y, z):
        self.position = (x, y, z)
        self.send(30, struct.pack('>dddB', x, y, z, 0))
        self.send(13)  # ClientTickEnd resets the one-position-per-client-tick gate.


def integer(reader, size, signed=True):
    return int.from_bytes(reader.take(size), 'big', signed=signed)


def paletted(reader, count, local_minimum, local_maximum, index=None):
    bits = reader.ubyte()
    if bits == 0:
        return reader.varint()
    if bits <= local_maximum:
        bits = max(bits, local_minimum)
        palette = [reader.varint() for _ in range(reader.varint())]
    else:
        palette = None
    per_word = 64 // bits
    data = reader.take(8 * ((count + per_word - 1) // per_word))
    if index is not None:
        offset = 8 * (index // per_word)
        word = int.from_bytes(data[offset:offset + 8], 'big')
        value = word >> ((index % per_word) * bits) & ((1 << bits) - 1)
        return palette[value] if palette is not None else value
    return None


def skip_nbt(reader, kind):
    if kind == 0:
        return
    if kind in (1, 2, 3, 4, 5, 6):
        reader.take((1, 2, 4, 8, 4, 8)[kind - 1])
    elif kind == 8:
        reader.take(integer(reader, 2, False))
    elif kind in (7, 11, 12):
        reader.take(integer(reader, 4) * {7: 1, 11: 4, 12: 8}[kind])
    elif kind == 9:
        entry = reader.ubyte()
        for _ in range(integer(reader, 4)):
            skip_nbt(reader, entry)
    elif kind == 10:
        while entry := reader.ubyte():
            reader.take(integer(reader, 2, False))
            skip_nbt(reader, entry)
    else:
        raise ValueError(f'invalid NBT tag {kind}')


def column(packet):
    return struct.unpack_from('>ii', packet.data)


def chunk_cell(packet, pos, min_y=-64):
    """Return the state and the optional embedded block-entity tag at pos."""
    x, y, z = pos
    assert column(packet) == (x >> 4, z >> 4)
    reader = Reader(packet.data[8:])
    for _ in range(reader.varint()):
        reader.varint()
        reader.take(8 * reader.varint())
    sections = Reader(reader.take(reader.varint()))
    index = ((y & 15) << 8) | ((z & 15) << 4) | (x & 15)
    for section in range(((y - min_y) >> 4) + 1):
        sections.take(4)  # nonempty blocks and nonempty fluids
        state = paletted(sections, 4096, 4, 8, index if section == ((y - min_y) >> 4) else None)
        paletted(sections, 64, 1, 3)
    tag = None
    for _ in range(reader.varint()):
        xz = reader.ubyte()
        tag_y = reader.short()
        reader.varint()
        start = reader.pos
        skip_nbt(reader, reader.ubyte())
        if (xz >> 4, tag_y, xz & 15) == (x & 15, y, z & 15):
            tag = reader.data[start:reader.pos]
    return state, tag


def varlong(reader):
    value = 0
    for shift in range(0, 70, 7):
        byte = reader.ubyte()
        value |= (byte & 0x7f) << shift
        if byte < 128:
            return value
    raise ValueError('VarLong too large')


def signed(value, bits):
    return value - (1 << bits) if value & (1 << (bits - 1)) else value


def updated_state(packet, pos):
    reader = Reader(packet.data)
    if packet.kind == BLOCK_UPDATE:
        if tuple(reader.pos_long()) == pos:
            return reader.varint()
    elif packet.kind == SECTION_UPDATE:
        section = integer(reader, 8, False)
        x = signed(section >> 42, 22)
        y = signed(section & ((1 << 20) - 1), 20)
        z = signed((section >> 20) & ((1 << 22) - 1), 22)
        if (x, y, z) == tuple(a >> 4 for a in pos):
            cell = ((pos[0] & 15) << 8) | ((pos[2] & 15) << 4) | (pos[1] & 15)
            for _ in range(reader.varint()):
                change = varlong(reader)
                if change & 0xfff == cell:
                    return change >> 12
    return None


def tag_at(packet, pos):
    if packet.kind == BLOCK_ENTITY:
        reader = Reader(packet.data)
        if tuple(reader.pos_long()) == pos:
            reader.varint()
            return reader.data[reader.pos:]
    return None
