export const buildingGroups = {
  power: {
    coal: 0x94c243eb,
    oil: 0x6e296726,
    wind: 0xb5923139,
    solar: 0x9122c84d,
    nuclear: 0xda87b792,
  },
  water: { tower: 0xbbf240b9, pump: 0x244f51db },
  sewage: { outlet: 0x4510d0e3, treatment: 0xdca517f2 },
  fire: { fireStation: 0xe917279c, largeFire: 0xa6b04f92 },
  police: { policeStation: 0x18d36a29, policePrecinct: 0xb40cd52c },
  health: { clinic: 0x30236d71, hospital: 0xe02667a4 },
  education: {
    school: 0xc264a268,
    highSchool: 0x64f750e2,
    university: 0x4250ae35,
  },
  transit: {
    shuttle: 0x0403377d,
    busStation: 0x4722ab8d,
    trainStation: 0x8357b87f,
  },
  waste: { landfill: 0x5244a0a5, recycling: 0xa16fc27f },
} as const;

export const developmentGroups = {
  specialization: { omegaFactory: 0x071d31e5 },
} as const;
