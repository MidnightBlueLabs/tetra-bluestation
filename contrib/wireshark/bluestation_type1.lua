local bluestation_type1 = Proto("bluestation_type1", "BlueStation Type-1 Capture")

local DIRECTION_NAMES = {
    [0] = "Downlink",
    [1] = "Uplink",
}

local LOGICAL_CHANNEL_NAMES = {
    [1] = "AACH",
    [2] = "BSCH",
    [3] = "BNCH",
    [4] = "SCH/HD",
    [5] = "SCH/F",
    [6] = "STCH",
    [7] = "SCH/HU",
    [8] = "TCH/S",
    [9] = "TCH/24",
    [10] = "TCH/48",
    [11] = "TCH/72",
    [12] = "BLCH",
    [13] = "CLCH",
}

local BLOCK_NAMES = {
    [0] = "Both",
    [1] = "Block1",
    [2] = "Block2",
    [255] = "Undefined",
}

local MAC_PDU_NAMES = {
    [0] = "MAC-RESOURCE / MAC-DATA",
    [1] = "MAC-FRAG / MAC-END",
    [2] = "Broadcast",
    [3] = "Supplementary / MAC-U-SIGNAL",
}

local BROADCAST_NAMES = {
    [0] = "SYSINFO",
    [1] = "ACCESS-DEFINE",
    [2] = "SYSINFO-DA",
}

local RESOURCE_ADDR_NAMES = {
    [0] = "NullPdu",
    [1] = "Ssi",
    [2] = "EventLabel",
    [3] = "Ussi",
    [4] = "Smi",
    [5] = "SsiAndEventLabel",
    [6] = "SsiAndUsageMarker",
    [7] = "SmiAndEventLabel",
}

local SYSINFO_OPT_NAMES = {
    [0] = "EvenMfDefForTsMode",
    [1] = "OddMfDefForTsMode",
    [2] = "DefaultDefForAccCodeA",
    [3] = "ExtServicesBroadcast",
}

local RES_REQ_NAMES = {
    [0] = "Req1Subslot",
    [1] = "Req1Slot",
    [2] = "Req2Slots",
    [3] = "Req3Slots",
    [4] = "Req4Slots",
    [5] = "Req5Slots",
    [6] = "Req6Slots",
    [7] = "Req8Slots",
    [8] = "Req10Slots",
    [9] = "Req13Slots",
    [10] = "Req17Slots",
    [11] = "Req24Slots",
    [12] = "Req34Slots",
    [13] = "Req51Slots",
    [14] = "Req68Slots",
    [15] = "ReqOver68",
}

local f = bluestation_type1.fields
f.magic = ProtoField.string("bluestation.type1.magic", "Magic")
f.version = ProtoField.uint8("bluestation.type1.version", "Version", base.DEC)
f.direction = ProtoField.uint8("bluestation.type1.direction", "Direction", base.DEC, DIRECTION_NAMES)
f.logical_channel = ProtoField.uint8("bluestation.type1.logical_channel", "Logical Channel", base.DEC, LOGICAL_CHANNEL_NAMES)
f.block = ProtoField.uint8("bluestation.type1.block", "Block", base.DEC, BLOCK_NAMES)
f.crc_pass = ProtoField.uint8("bluestation.type1.crc_pass", "CRC Pass", base.DEC, { [0] = "false", [1] = "true" })
f.reserved = ProtoField.uint8("bluestation.type1.reserved", "Reserved", base.HEX)
f.hyperframe = ProtoField.uint16("bluestation.type1.hyperframe", "Hyperframe", base.DEC)
f.multiframe = ProtoField.uint8("bluestation.type1.multiframe", "Multiframe", base.DEC)
f.frame = ProtoField.uint8("bluestation.type1.frame", "Frame", base.DEC)
f.timeslot = ProtoField.uint8("bluestation.type1.timeslot", "Timeslot", base.DEC)
f.scrambling_code = ProtoField.uint32("bluestation.type1.scrambling_code", "Scrambling Code", base.HEX)
f.bit_length = ProtoField.uint16("bluestation.type1.bit_length", "Bit Length", base.DEC)
f.bits = ProtoField.string("bluestation.type1.bits", "Type-1 Bits")

local function lookup(tbl, value, fallback)
    if tbl[value] ~= nil then
        return tbl[value]
    end
    return fallback or tostring(value)
end

local function bits_to_uint(bits, start_bit, bit_len)
    if start_bit < 0 or bit_len < 0 then
        return nil
    end
    local stop = start_bit + bit_len
    if stop > #bits then
        return nil
    end

    local value = 0
    for idx = start_bit + 1, stop do
        local c = bits:byte(idx)
        if c ~= 48 and c ~= 49 then
            return nil
        end
        value = (value * 2) + (c - 48)
    end
    return value
end

local function bits_slice(bits, start_bit, bit_len)
    if start_bit < 0 or bit_len < 0 then
        return nil
    end
    local stop = start_bit + bit_len
    if stop > #bits then
        return nil
    end
    return bits:sub(start_bit + 1, stop)
end

local function add_text(tree, range, text)
    tree:add(range, text)
end

local function add_flag_summary(tree, range, label, flags)
    local enabled = {}
    for _, flag in ipairs(flags) do
        if flag.enabled then
            table.insert(enabled, flag.name)
        end
    end
    if #enabled == 0 then
        add_text(tree, range, label .. ": none")
    else
        add_text(tree, range, label .. ": " .. table.concat(enabled, ", "))
    end
end

local function parse_access_assign(bits, tree, range, is_frame18)
    local header = bits_to_uint(bits, 0, 2)
    local field1 = bits_to_uint(bits, 2, 6)
    local field2 = bits_to_uint(bits, 8, 6)
    if header == nil or field1 == nil or field2 == nil then
        add_text(tree, range, "ACCESS-ASSIGN truncated")
        return
    end

    local aa_tree = tree:add(range, string.format("ACCESS-ASSIGN%s", is_frame18 and " FR18" or ""))
    add_text(aa_tree, range, string.format("Header: %u", header))

    if is_frame18 then
        if header <= 2 then
            local ul_usage = ({ [0] = "CommonOnly", [1] = "CommonAndAssigned", [2] = "AssignedOnly" })[header]
            add_text(aa_tree, range, "UL usage: " .. ul_usage)
            add_text(aa_tree, range, string.format("Access field 1: code=%u base_frame_len=%u", math.floor(field1 / 16), field1 % 16))
            add_text(aa_tree, range, string.format("Access field 2: code=%u base_frame_len=%u", math.floor(field2 / 16), field2 % 16))
        else
            local traffic = field1 >= 4 and ("Traffic(" .. field1 .. ")") or ("Invalid(" .. field1 .. ")")
            add_text(aa_tree, range, "UL traffic usage: " .. traffic)
            add_text(aa_tree, range, string.format("Shared access field: code=%u base_frame_len=%u", math.floor(field2 / 16), field2 % 16))
        end
        return
    end

    if header == 0 then
        add_text(aa_tree, range, "DL usage: CommonControl")
        add_text(aa_tree, range, "UL usage: CommonOnly")
        add_text(aa_tree, range, string.format("Access field 1: code=%u base_frame_len=%u", math.floor(field1 / 16), field1 % 16))
        add_text(aa_tree, range, string.format("Access field 2: code=%u base_frame_len=%u", math.floor(field2 / 16), field2 % 16))
    elseif header == 1 then
        local dl_usage = ({ [0] = "Unallocated", [1] = "AssignedControl", [2] = "CommonControl", [3] = "CommonAndAssigned" })[field1]
        if dl_usage == nil then
            dl_usage = "Traffic(" .. field1 .. ")"
        end
        add_text(aa_tree, range, "DL usage: " .. dl_usage)
        add_text(aa_tree, range, "UL usage: CommonAndAssigned")
        add_text(aa_tree, range, string.format("Shared access field: code=%u base_frame_len=%u", math.floor(field2 / 16), field2 % 16))
    elseif header == 2 then
        local dl_usage = ({ [0] = "Unallocated", [1] = "AssignedControl", [2] = "CommonControl", [3] = "CommonAndAssigned" })[field1]
        if dl_usage == nil then
            dl_usage = "Traffic(" .. field1 .. ")"
        end
        add_text(aa_tree, range, "DL usage: " .. dl_usage)
        add_text(aa_tree, range, "UL usage: AssignedOnly")
        add_text(aa_tree, range, string.format("Shared access field: code=%u base_frame_len=%u", math.floor(field2 / 16), field2 % 16))
    else
        local dl_usage = ({ [0] = "Unallocated", [1] = "AssignedControl", [2] = "CommonControl", [3] = "CommonAndAssigned" })[field1]
        if dl_usage == nil then
            dl_usage = "Traffic(" .. field1 .. ")"
        end
        local ul_usage = field2 == 0 and "Unallocated" or (field2 >= 4 and ("Traffic(" .. field2 .. ")") or ("Invalid(" .. field2 .. ")"))
        add_text(aa_tree, range, "DL usage: " .. dl_usage)
        add_text(aa_tree, range, "UL usage: " .. ul_usage)
    end
end

local function parse_bsch(bits, tree, range)
    local sync_tree = tree:add(range, "BSCH: MAC-SYNC + D-MLE-SYNC")
    local system_code = bits_to_uint(bits, 0, 4)
    local colour_code = bits_to_uint(bits, 4, 6)
    local timeslot = bits_to_uint(bits, 10, 2)
    local frame = bits_to_uint(bits, 12, 5)
    local multiframe = bits_to_uint(bits, 17, 6)
    local sharing_mode = bits_to_uint(bits, 23, 2)
    local ts_reserved = bits_to_uint(bits, 25, 3)
    local u_plane_dtx = bits_to_uint(bits, 28, 1)
    local frame18_ext = bits_to_uint(bits, 29, 1)
    local reserved = bits_to_uint(bits, 30, 1)

    if system_code == nil or colour_code == nil then
        add_text(sync_tree, range, "MAC-SYNC truncated")
        return
    end

    add_text(sync_tree, range, string.format("MAC-SYNC: system_code=%u colour_code=%u time=%u/%u/%u sharing_mode=%u ts_reserved_frames=%u u_plane_dtx=%s frame_18_ext=%s reserved=%u",
        system_code,
        colour_code,
        (multiframe or 0),
        (frame or 0),
        ((timeslot or 0) + 1),
        sharing_mode or 0,
        ts_reserved or 0,
        u_plane_dtx == 1 and "true" or "false",
        frame18_ext == 1 and "true" or "false",
        reserved or 0
    ))

    local mcc = bits_to_uint(bits, 31, 10)
    local mnc = bits_to_uint(bits, 41, 14)
    local neighbor_cell_broadcast = bits_to_uint(bits, 55, 2)
    local cell_load_ca = bits_to_uint(bits, 57, 2)
    local late_entry_supported = bits_to_uint(bits, 59, 1)
    if mcc ~= nil then
        add_text(sync_tree, range, string.format("D-MLE-SYNC: mcc=%u mnc=%u neighbor_cell_broadcast=%u cell_load_ca=%u late_entry_supported=%s",
            mcc,
            mnc or 0,
            neighbor_cell_broadcast or 0,
            cell_load_ca or 0,
            late_entry_supported == 1 and "true" or "false"
        ))
    end
end

local function parse_bs_service_details(bits, start_bit, tree, range)
    local flags = {
        { name = "registration", enabled = bits_to_uint(bits, start_bit + 0, 1) == 1 },
        { name = "deregistration", enabled = bits_to_uint(bits, start_bit + 1, 1) == 1 },
        { name = "priority_cell", enabled = bits_to_uint(bits, start_bit + 2, 1) == 1 },
        { name = "no_minimum_mode", enabled = bits_to_uint(bits, start_bit + 3, 1) == 1 },
        { name = "migration", enabled = bits_to_uint(bits, start_bit + 4, 1) == 1 },
        { name = "system_wide_services", enabled = bits_to_uint(bits, start_bit + 5, 1) == 1 },
        { name = "voice_service", enabled = bits_to_uint(bits, start_bit + 6, 1) == 1 },
        { name = "circuit_mode_data_service", enabled = bits_to_uint(bits, start_bit + 7, 1) == 1 },
        { name = "sndcp_service", enabled = bits_to_uint(bits, start_bit + 9, 1) == 1 },
        { name = "aie_service", enabled = bits_to_uint(bits, start_bit + 10, 1) == 1 },
        { name = "advanced_link", enabled = bits_to_uint(bits, start_bit + 11, 1) == 1 },
    }
    add_flag_summary(tree, range, "BS service details", flags)
end

local function parse_sysinfo_optional(bits, option_field, tree, range)
    if option_field == 0 or option_field == 1 then
        local common_frames = bits_slice(bits, 62, 20)
        if common_frames ~= nil then
            add_text(tree, range, string.format("%s: %s",
                lookup(SYSINFO_OPT_NAMES, option_field, "Unknown"),
                common_frames
            ))
        end
    elseif option_field == 2 then
        local imm = bits_to_uint(bits, 62, 4)
        local wt = bits_to_uint(bits, 66, 4)
        local nu = bits_to_uint(bits, 70, 4)
        local fl_factor = bits_to_uint(bits, 74, 1)
        local ts_ptr = bits_to_uint(bits, 75, 4)
        local min_pdu_prio = bits_to_uint(bits, 79, 3)
        add_text(tree, range, string.format(
            "DefaultDefForAccCodeA: imm=%u wt=%u nu=%u fl_factor=%s ts_ptr=%u min_pdu_prio=%u",
            imm or 0,
            wt or 0,
            nu or 0,
            fl_factor == 1 and "true" or "false",
            ts_ptr or 0,
            min_pdu_prio or 0
        ))
    elseif option_field == 3 then
        local auth_required = bits_to_uint(bits, 62, 1)
        local class1_supported = bits_to_uint(bits, 63, 1)
        local class3_marker = bits_to_uint(bits, 64, 1)
        local security_tail = bits_slice(bits, 65, 5) or ""
        local sdstl_addressing_method = bits_to_uint(bits, 70, 2)
        local gck_supported = bits_to_uint(bits, 72, 1)
        local section = bits_to_uint(bits, 73, 2)
        local section_data = bits_to_uint(bits, 75, 7)
        add_text(tree, range, string.format(
            "ExtServicesBroadcast: auth_required=%s class1_supported=%s class3_marker=%u security_tail=%s sdstl_addressing_method=%u gck_supported=%s section=%u section_data=%u",
            auth_required == 1 and "true" or "false",
            class1_supported == 1 and "true" or "false",
            class3_marker or 0,
            security_tail,
            sdstl_addressing_method or 0,
            gck_supported == 1 and "true" or "false",
            section or 0,
            section_data or 0
        ))
    end
end

local function parse_bnch(bits, tree, range)
    local bnch_tree = tree:add(range, "BNCH: MAC-SYSINFO + D-MLE-SYSINFO")
    local pdu_type = bits_to_uint(bits, 0, 2)
    local pdu_subtype = bits_to_uint(bits, 2, 2)
    if pdu_type ~= 2 or pdu_subtype ~= 0 then
        add_text(bnch_tree, range, string.format("Unexpected MAC-SYSINFO header: pdu_type=%s subtype=%s", tostring(pdu_type), tostring(pdu_subtype)))
        return
    end

    local main_carrier = bits_to_uint(bits, 4, 12)
    local freq_band = bits_to_uint(bits, 16, 4)
    local freq_offset_index = bits_to_uint(bits, 20, 2)
    local duplex_spacing = bits_to_uint(bits, 22, 3)
    local reverse_operation = bits_to_uint(bits, 25, 1)
    local num_of_csch = bits_to_uint(bits, 26, 2)
    local ms_txpwr_max_cell = bits_to_uint(bits, 28, 3)
    local rxlev_access_min = bits_to_uint(bits, 31, 4)
    local access_parameter = bits_to_uint(bits, 35, 4)
    local radio_dl_timeout = bits_to_uint(bits, 39, 4)
    local has_cck_field = bits_to_uint(bits, 43, 1)
    local cck_or_hfn = bits_to_uint(bits, 44, 16)
    local option_field = bits_to_uint(bits, 60, 2)
    add_text(bnch_tree, range, string.format(
        "MAC-SYSINFO: main_carrier=%u freq_band=%u freq_offset_index=%u duplex_spacing=%u reverse_operation=%s num_of_csch=%u ms_txpwr_max_cell=%u rxlev_access_min=%u access_parameter=%u radio_dl_timeout=%u %s=%u option_field=%s",
        main_carrier or 0,
        freq_band or 0,
        freq_offset_index or 0,
        duplex_spacing or 0,
        reverse_operation == 1 and "true" or "false",
        num_of_csch or 0,
        ms_txpwr_max_cell or 0,
        rxlev_access_min or 0,
        access_parameter or 0,
        radio_dl_timeout or 0,
        has_cck_field == 1 and "cck_id" or "hyperframe_number",
        cck_or_hfn or 0,
        lookup(SYSINFO_OPT_NAMES, option_field, "Unknown")
    ))

    if option_field ~= nil then
        parse_sysinfo_optional(bits, option_field, bnch_tree, range)
    end

    local location_area = bits_to_uint(bits, 82, 14)
    local subscriber_class = bits_to_uint(bits, 96, 16)
    if location_area ~= nil and subscriber_class ~= nil then
        add_text(bnch_tree, range, string.format("D-MLE-SYSINFO: location_area=%u subscriber_class=0x%04X", location_area, subscriber_class))
        parse_bs_service_details(bits, 112, bnch_tree, range)
    end
end

local function parse_mac_resource(bits, tree, range)
    local fill_bits = bits_to_uint(bits, 2, 1)
    local pos_of_grant = bits_to_uint(bits, 3, 1)
    local encryption_mode = bits_to_uint(bits, 4, 2)
    local random_access_flag = bits_to_uint(bits, 6, 1)
    local length_ind = bits_to_uint(bits, 7, 6)
    local addr_type = bits_to_uint(bits, 13, 3)
    if fill_bits == nil or addr_type == nil then
        add_text(tree, range, "MAC-RESOURCE truncated")
        return
    end

    local mr_tree = tree:add(range, "MAC-RESOURCE")
    add_text(mr_tree, range, string.format("fill_bits=%s pos_of_grant=%u encryption_mode=%u random_access_flag=%s length_ind=%u addr_type=%s",
        fill_bits == 1 and "true" or "false",
        pos_of_grant or 0,
        encryption_mode or 0,
        random_access_flag == 1 and "true" or "false",
        length_ind or 0,
        lookup(RESOURCE_ADDR_NAMES, addr_type, "Unknown")
    ))

    local pos = 16
    if addr_type == 1 or addr_type == 3 or addr_type == 4 then
        local addr = bits_to_uint(bits, pos, 24)
        add_text(mr_tree, range, string.format("Address: %u", addr or 0))
        pos = pos + 24
    elseif addr_type == 2 then
        local event_label = bits_to_uint(bits, pos, 10)
        add_text(mr_tree, range, string.format("Event label: %u", event_label or 0))
        pos = pos + 10
    elseif addr_type == 5 or addr_type == 7 then
        local addr = bits_to_uint(bits, pos, 24)
        local event_label = bits_to_uint(bits, pos + 24, 10)
        add_text(mr_tree, range, string.format("Address: %u event_label=%u", addr or 0, event_label or 0))
        pos = pos + 34
    elseif addr_type == 6 then
        local addr = bits_to_uint(bits, pos, 24)
        local usage_marker = bits_to_uint(bits, pos + 24, 6)
        add_text(mr_tree, range, string.format("Address: %u usage_marker=%u", addr or 0, usage_marker or 0))
        pos = pos + 30
    else
        add_text(mr_tree, range, "Null PDU")
        return
    end

    local power_control_flag = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if power_control_flag == 1 then
        local power_control_element = bits_to_uint(bits, pos, 4)
        add_text(mr_tree, range, string.format("Power control: %u", power_control_element or 0))
        pos = pos + 4
    end

    local slot_granting_flag = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if slot_granting_flag == 1 then
        local slot_grant = bits_to_uint(bits, pos, 8)
        add_text(mr_tree, range, string.format("Basic slotgrant: 0x%02X", slot_grant or 0))
        pos = pos + 8
    end

    local chan_alloc_flag = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if chan_alloc_flag == 1 then
        local chan_alloc_bits = bits_slice(bits, pos, #bits - pos)
        add_text(mr_tree, range, "Channel allocation element: " .. (chan_alloc_bits or "<truncated>"))
        pos = #bits
    end

    if pos < #bits then
        local tm_sdu = bits_slice(bits, pos, #bits - pos)
        if tm_sdu ~= nil and tm_sdu ~= "" then
            add_text(mr_tree, range, "TM-SDU bits: " .. tm_sdu)
        end
    end
end

local function parse_mac_data(bits, tree, range)
    local fill_bits = bits_to_uint(bits, 2, 1)
    local encrypted = bits_to_uint(bits, 3, 1)
    local addr_type = bits_to_uint(bits, 4, 2)
    if fill_bits == nil or addr_type == nil then
        add_text(tree, range, "MAC-DATA truncated")
        return
    end

    local md_tree = tree:add(range, "MAC-DATA")
    add_text(md_tree, range, string.format("fill_bits=%s encrypted=%s addr_type=%u",
        fill_bits == 1 and "true" or "false",
        encrypted == 1 and "true" or "false",
        addr_type
    ))

    local pos = 6
    if addr_type == 0 or addr_type == 2 or addr_type == 3 then
        local addr = bits_to_uint(bits, pos, 24)
        add_text(md_tree, range, string.format("Address: %u", addr or 0))
        pos = pos + 24
    elseif addr_type == 1 then
        local event_label = bits_to_uint(bits, pos, 10)
        add_text(md_tree, range, string.format("Event label: %u", event_label or 0))
        pos = pos + 10
    end

    local length_ind_or_cap_req = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if length_ind_or_cap_req == 0 then
        local length_ind = bits_to_uint(bits, pos, 6)
        add_text(md_tree, range, string.format("Length indication: %u", length_ind or 0))
        pos = pos + 6
    else
        local frag_flag = bits_to_uint(bits, pos, 1)
        local reservation_req = bits_to_uint(bits, pos + 1, 4)
        add_text(md_tree, range, string.format("Fragmentation: frag_flag=%s reservation_requirement=%s",
            frag_flag == 1 and "true" or "false",
            lookup(RES_REQ_NAMES, reservation_req, tostring(reservation_req))
        ))
        pos = pos + 6
    end

    if pos < #bits then
        local sdu_bits = bits_slice(bits, pos, #bits - pos)
        if sdu_bits ~= nil and sdu_bits ~= "" then
            add_text(md_tree, range, "TM-SDU bits: " .. sdu_bits)
        end
    end
end

local function parse_mac_end_or_frag(bits, tree, range, direction, logical_channel)
    if logical_channel == 7 then
        local mac_pdu_type = bits_to_uint(bits, 0, 1)
        if mac_pdu_type == 1 then
            local fill_bits = bits_to_uint(bits, 1, 1)
            local length_ind_or_cap_req = bits_to_uint(bits, 2, 1)
            if length_ind_or_cap_req == 0 then
                local length_ind = bits_to_uint(bits, 3, 4)
                add_text(tree, range, string.format("MAC-END-HU: fill_bits=%s length_ind=%u",
                    fill_bits == 1 and "true" or "false",
                    length_ind or 0
                ))
            else
                local reservation_req = bits_to_uint(bits, 3, 4)
                add_text(tree, range, string.format("MAC-END-HU: fill_bits=%s reservation_requirement=%s",
                    fill_bits == 1 and "true" or "false",
                    lookup(RES_REQ_NAMES, reservation_req, tostring(reservation_req))
                ))
            end
        else
            add_text(tree, range, "SCH/HU subtype not decoded; raw bits follow")
        end
        return
    end

    local subtype = bits_to_uint(bits, 2, 1)
    local fill_bits = bits_to_uint(bits, 3, 1)
    if subtype == nil or fill_bits == nil then
        add_text(tree, range, "MAC-END/FRAG truncated")
        return
    end

    if subtype == 0 then
        add_text(tree, range, string.format("MAC-FRAG (%s): fill_bits=%s",
            direction == 0 and "downlink" or "uplink",
            fill_bits == 1 and "true" or "false"
        ))
        if #bits > 4 then
            local sdu_bits = bits_slice(bits, 4, #bits - 4)
            if sdu_bits ~= nil and sdu_bits ~= "" then
                add_text(tree, range, "TM-SDU bits: " .. sdu_bits)
            end
        end
        return
    end

    if direction == 0 then
        local pos_of_grant = bits_to_uint(bits, 4, 1)
        local length_ind = bits_to_uint(bits, 5, 6)
        add_text(tree, range, string.format("MAC-END-DL: fill_bits=%s pos_of_grant=%u length_ind=%u",
            fill_bits == 1 and "true" or "false",
            pos_of_grant or 0,
            length_ind or 0
        ))
    else
        local length_ind_cap_req = bits_to_uint(bits, 4, 6)
        if length_ind_cap_req ~= nil and length_ind_cap_req >= 48 then
            local reservation_req = length_ind_cap_req % 16
            add_text(tree, range, string.format("MAC-END-UL: fill_bits=%s reservation_requirement=%s",
                fill_bits == 1 and "true" or "false",
                lookup(RES_REQ_NAMES, reservation_req, tostring(reservation_req))
            ))
        else
            add_text(tree, range, string.format("MAC-END-UL: fill_bits=%s length_ind=%u",
                fill_bits == 1 and "true" or "false",
                length_ind_cap_req or 0
            ))
        end
    end
end

local function parse_mac_u_signal(bits, tree, range)
    local second_half_stolen = bits_to_uint(bits, 2, 1)
    add_text(tree, range, string.format("MAC-U-SIGNAL: second_half_stolen=%s",
        second_half_stolen == 1 and "true" or "false"
    ))
end

local function parse_generic_mac(bits, tree, range, direction, logical_channel)
    if #bits < 2 then
        add_text(tree, range, "MAC block truncated")
        return
    end

    if logical_channel == 7 and #bits >= 1 and bits_to_uint(bits, 0, 1) == 1 then
        parse_mac_end_or_frag(bits, tree, range, direction, logical_channel)
        return
    end

    local mac_pdu_type = bits_to_uint(bits, 0, 2)
    local mac_tree = tree:add(range, "MAC PDU: " .. lookup(MAC_PDU_NAMES, mac_pdu_type, "Unknown"))
    if mac_pdu_type == 0 then
        if direction == 0 then
            parse_mac_resource(bits, mac_tree, range)
        else
            parse_mac_data(bits, mac_tree, range)
        end
    elseif mac_pdu_type == 1 then
        parse_mac_end_or_frag(bits, mac_tree, range, direction, logical_channel)
    elseif mac_pdu_type == 2 then
        local broadcast_type = bits_to_uint(bits, 2, 2)
        add_text(mac_tree, range, "Broadcast subtype: " .. lookup(BROADCAST_NAMES, broadcast_type, "Unknown"))
        if logical_channel == 3 and broadcast_type == 0 then
            parse_bnch(bits, mac_tree, range)
        elseif #bits > 4 then
            local tm_sdu = bits_slice(bits, 4, #bits - 4)
            add_text(mac_tree, range, "Broadcast payload bits: " .. (tm_sdu or "<truncated>"))
        end
    elseif mac_pdu_type == 3 then
        parse_mac_u_signal(bits, mac_tree, range)
        if #bits > 3 then
            local tail_bits = bits_slice(bits, 3, #bits - 3)
            if tail_bits ~= nil and tail_bits ~= "" then
                add_text(mac_tree, range, "Remaining bits: " .. tail_bits)
            end
        end
    else
        add_text(mac_tree, range, "Raw bits: " .. bits)
    end
end

local function decode_type1(bits, tree, range, direction, logical_channel, frame)
    if logical_channel == 1 then
        parse_access_assign(bits, tree, range, frame == 18)
    elseif logical_channel == 2 then
        parse_bsch(bits, tree, range)
    elseif logical_channel == 3 then
        parse_bnch(bits, tree, range)
    elseif logical_channel == 4 or logical_channel == 5 or logical_channel == 6 or logical_channel == 7 then
        parse_generic_mac(bits, tree, range, direction, logical_channel)
    elseif logical_channel >= 8 and logical_channel <= 11 then
        add_text(tree, range, "Traffic channel type-1 bits: voice/data payload not further dissected")
    else
        add_text(tree, range, "Logical channel not dissected; raw type-1 bits preserved")
    end
end

local function dissect_impl(tvb, pinfo, tree)
    if tvb:len() < 21 then
        return false
    end
    if tvb(0, 4):string() ~= "TBSW" then
        return false
    end

    local version = tvb(4, 1):uint()
    local direction = tvb(5, 1):uint()
    local logical_channel = tvb(6, 1):uint()
    local block = tvb(7, 1):uint()
    local crc_pass = tvb(8, 1):uint()
    local hyperframe = tvb(10, 2):uint()
    local multiframe = tvb(12, 1):uint()
    local frame = tvb(13, 1):uint()
    local timeslot = tvb(14, 1):uint()
    local bit_length = tvb(19, 2):uint()

    local available_bit_bytes = tvb:len() - 21
    local bit_bytes = math.min(bit_length, available_bit_bytes)
    local payload_range = tvb(21, bit_bytes)
    local bits = payload_range:string()

    pinfo.cols.protocol = "BST1"
    pinfo.cols.info = string.format("%s %s %s %s",
        lookup(DIRECTION_NAMES, direction, "Unknown"),
        lookup(LOGICAL_CHANNEL_NAMES, logical_channel, "Unknown"),
        lookup(BLOCK_NAMES, block, "Unknown"),
        crc_pass == 1 and "CRC ok" or "CRC fail"
    )

    local subtree = tree:add(bluestation_type1, tvb(), "BlueStation Type-1 Capture")
    subtree:add(f.magic, tvb(0, 4))
    subtree:add(f.version, tvb(4, 1))
    subtree:add(f.direction, tvb(5, 1))
    subtree:add(f.logical_channel, tvb(6, 1))
    subtree:add(f.block, tvb(7, 1))
    subtree:add(f.crc_pass, tvb(8, 1))
    subtree:add(f.reserved, tvb(9, 1))
    subtree:add(f.hyperframe, tvb(10, 2))
    subtree:add(f.multiframe, tvb(12, 1))
    subtree:add(f.frame, tvb(13, 1))
    subtree:add(f.timeslot, tvb(14, 1))
    subtree:add(f.scrambling_code, tvb(15, 4))
    subtree:add(f.bit_length, tvb(19, 2))
    subtree:add(f.bits, payload_range)

    if bit_bytes < bit_length then
        add_text(subtree, payload_range, string.format("Truncated capture payload: expected %u bit characters, got %u", bit_length, bit_bytes))
    end

    decode_type1(bits, subtree, payload_range, direction, logical_channel, frame)
    return true
end

function bluestation_type1.dissector(tvb, pinfo, tree)
    if dissect_impl(tvb, pinfo, tree) then
        return tvb:len()
    end
    return 0
end

bluestation_type1:register_heuristic("udp", dissect_impl)
DissectorTable.get("udp.port"):add(42069, bluestation_type1)
