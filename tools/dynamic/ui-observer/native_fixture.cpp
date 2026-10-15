// Independent x86 fixture. Never loaded into SimCity.
#include <windows.h>
#include <cstdio>
#include <cstdint>
#include <cstring>
#include <cstddef>
#include <thread>
#include <atomic>

struct Binding { void* vtable; void* host; };
struct StringImpl { unsigned refs, length; const wchar_t* chars; };
struct String { StringImpl* impl; };
struct Chrome { void* vtable; void* page; };
static std::atomic<unsigned> calls{0}, bad{0}, logs{0};
static int host, page, table;
static Binding binding{&table, &host};
struct Structure { unsigned a, b; unsigned char type; };
struct Cell { void* table; Structure* structure; unsigned length; StringImpl* impl; unsigned fibers; };
struct alignas(8) Value { void* table; unsigned pad; void* payload; unsigned tag; void* exec; void* iterator; };
static_assert(sizeof(Value) == 24);
static Structure structure{0, 0, 5};
static StringImpl commandText{1, 19, L"camera fixture test"};
static Cell flat{&table, &structure, 19, &commandText, 0};
static Cell rope{&table, &structure, 19, nullptr, 2};
static Cell huge{&table, &structure, 1048577, nullptr, 0};
static Value values[] = {{&table, 0, &flat, 0xfffffffb, nullptr, nullptr},
                         {&table, 0, &rope, 0xfffffffb, nullptr, nullptr},
                         {&table, 0, &huge, 0xfffffffb, nullptr, nullptr},
                         {&table, 0, (void*)1, 0xfffffffb, nullptr, nullptr}};
static Chrome chrome{&table, &page};
static int objectTable;
static unsigned objectShape[20]{};
static StringImpl propertyName{1, 9, L"regionBox"};
static int arrayTable;
struct RawValue { unsigned payload, tag; };
struct Storage { unsigned length, used; void* sparse; void* lazy; void* alloc; unsigned capacity; RawValue items[2]; };
struct Array { void* table; unsigned char padding[44]; unsigned capacity, bias; Storage* storage; };
static_assert(offsetof(Array, storage) == 0x38);
static Storage storage{2, 2, nullptr, nullptr, nullptr, 0,
    {{246368015, 0xffffffff}, {(unsigned)&flat, 0xfffffffb}}};
static Array array{&arrayTable, {}, 2, 0, &storage};
static Value eventValue{&table, 0, &array, 0xfffffffb, nullptr, nullptr};
static Value* eventRef = &eventValue;
struct Object { void* table; void* shape; RawValue* storage; };
static RawValue propertyValue{1026, 0xffffffff};
static Object object{&objectTable, objectShape, &propertyValue};
struct Entry { StringImpl* key; unsigned offset, attributes; void* specific; };
struct TableData { unsigned index[16]; Entry entries[2]; };
static TableData tableData{{}, {{&propertyName, 0, 0, nullptr}, {(StringImpl*)1, 0, 0, nullptr}}};
struct PropertyTable { unsigned size, mask; void* data; unsigned live, deleted; void* deletedOffsets; };
static PropertyTable properties{16, 15, &tableData, 1, 1, nullptr};

__declspec(noinline) bool __fastcall reverse(void* self, void*, void* functionRef, void* resultRef, Value** arg) {
    if (!self || functionRef != &table || resultRef || arg != &eventRef) bad++;
    return arg == &eventRef;
}
__declspec(naked) bool __fastcall reverseCaller(void*, void*, void*, void*, Value**) {
    __asm {
        push dword ptr [esp + 12]
        push dword ptr [esp + 12]
        push dword ptr [esp + 12]
        call reverse
        ret 12
    }
}


__declspec(noinline) bool __fastcall invoke(Binding* self, void*, const char* name,
                                            void* args, unsigned count, void* result) {
    calls++;
    if (self != &binding || (name[0] != 'P' && name[0] != 'R') || args != values || count != 4 || !result) bad++;
    Value* output = static_cast<Value*>(result);
    *output = values[0];
    output->tag = 0xffffffff;
    output->payload = reinterpret_cast<void*>(GetCurrentThreadId());
    static thread_local unsigned serial = 0;
    if (++serial % 2 == 0) {
        const double number = 42.25;
        std::memcpy(&output->payload, &number, sizeof(number));
    }
    return count == 4 && self == &binding;
}

__declspec(noinline) void __fastcall log_message(Chrome* self, void*, unsigned source,
    unsigned type, unsigned level, const String& message, unsigned line, const String& uri) {
    logs++;
    if (self != &chrome || source != 1 || type != 2 || level != 3 || line != 42 ||
        message.impl->chars[0] != L'f' || uri.impl->length != 10) bad++;
}

static void work(unsigned count) {
    Value output{};
    if (!reverseCaller(&host, nullptr, &table, nullptr, &eventRef)) bad++;
    if (!reverse(&host, nullptr, &table, nullptr, &eventRef)) bad++;
    if (!reverseCaller(&page, nullptr, &table, nullptr, &eventRef)) bad++;
    for (unsigned i = 0; i < count; ++i) {
        if (!invoke(&binding, nullptr, (i % 50 == 0 ? "PostGameCommand" : "RequestGameData"), values, 4, &output)) bad++;
        if (output.tag == 0xffffffff) {
            if ((unsigned)output.payload != GetCurrentThreadId()) bad++;
        } else {
            double number;
            std::memcpy(&number, &output.payload, sizeof(number));
            if (number != 42.25) bad++;
        }
        Sleep(2);
    }
}

int main() {
    std::printf("{\"base\":\"%p\",\"invoke\":\"%p\",\"log\":\"%p\",\"object\":\"%p\",\"vtable\":\"%p\",\"host\":\"%p\",\"page\":\"%p\",\"event\":\"%p\",\"eventCaller\":\"%p\",\"arrayTable\":\"%p\",\"objectTable\":\"%p\"}\n",
        GetModuleHandleW(nullptr), &invoke, &log_message, &binding, &table, &host, &page, &reverse, &reverseCaller, &arrayTable, &objectTable);
    std::fflush(stdout);
    std::getchar();
    StringImpl message{1, 15, L"fixture message"}, uri{1, 10, L"fixture.js"};
    String a{&message}, b{&uri};
    log_message(&chrome, nullptr, 1, 2, 3, a, 42, b);
    storage.items[1].payload = (unsigned)&array;
    reverseCaller(&host, nullptr, &table, nullptr, &eventRef);
    storage.items[1].payload = (unsigned)&flat;
    storage.sparse = &host;
    reverseCaller(&host, nullptr, &table, nullptr, &eventRef);
    storage.sparse = nullptr;
    storage.length = 1048577;
    reverseCaller(&host, nullptr, &table, nullptr, &eventRef);
    storage.length = 2;
    objectShape[0x38 / 4] = 1;
    objectShape[0x34 / 4] = (unsigned)&properties;
    storage.items[1].payload = (unsigned)&object;
    reverseCaller(&host, nullptr, &table, nullptr, &eventRef);
    objectShape[0x34 / 4] = 0;
    objectShape[0x20 / 4] = (unsigned)&propertyName;
    reverseCaller(&host, nullptr, &table, nullptr, &eventRef);
    objectShape[0x44 / 4] = 2;
    reverseCaller(&host, nullptr, &table, nullptr, &eventRef);
    objectShape[0x44 / 4] = 0;
    propertyValue = {(unsigned)&object, 0xfffffffb};
    reverseCaller(&host, nullptr, &table, nullptr, &eventRef);
    storage.items[1].payload = (unsigned)&flat;
    std::thread one(work, 250), two(work, 250);
    one.join(); two.join();
    std::printf("{\"calls\":%u,\"bad\":%u,\"logs\":%u}\n", calls.load(), bad.load(), logs.load());
    std::fflush(stdout);
    std::getchar();
    work(20);
    std::printf("{\"afterDetachCalls\":%u,\"bad\":%u}\n", calls.load(), bad.load());
    return bad.load() ? 1 : 0;
}
